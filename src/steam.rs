use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use chrono::Local;
use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, watch};
use steam_vent::auth::{
    AuthConfirmationHandler, DeviceConfirmationHandler, FileGuardDataStore,
    SharedSecretAuthConfirmationHandler, UserProvidedAuthConfirmationHandler,
};
use steam_vent::{Connection, ConnectionTrait, ServerList};
use steam_vent_proto_steam::enums::EPersonaStateFlag;
use steam_vent_proto_steam::steammessages_chat_steamclient::CChatRoom_IncomingChatMessage_Notification;
use steam_vent_proto_steam::steammessages_friendmessages_steamclient::{
    CFriendMessages_IncomingMessage_Notification, CFriendMessages_SendMessage_Request,
    CFriendsMessages_GetActiveMessageSessions_Request,
};
use steam_vent_proto_steam::steammessages_clientserver::{
    cmsg_client_games_played::GamePlayed, CMsgClientGamesPlayed,
};
use steam_vent_proto_steam::steammessages_clientserver_2::{
    CMsgClientItemAnnouncements, CMsgClientPlayingSessionState,
};
use steam_vent_proto_steam::steammessages_clientserver_friends::{
    CMsgClientChangeStatus, CMsgClientFriendMsg, CMsgClientFriendMsgIncoming,
};
use crate::data::{Account, AccountStatus, GameEntry, PersonaState};
use crate::debug_log::{self, PacketCategory, PacketDirection};
use crate::logger;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize Tokio runtime")
    })
}

pub enum BoostCommand {
    UpdatePresence {
        games: Vec<GameEntry>,
        custom_title: String,
        persona_state: PersonaState,
        afk_reply_custom: Option<String>,
    },
    Stop,
}

struct ActiveWorker {
    guard_tx: Option<DuplexStream>,
    stop_tx: watch::Sender<bool>,
    cmd_tx: mpsc::UnboundedSender<BoostCommand>,
}

static WORKERS: OnceLock<Mutex<HashMap<u64, ActiveWorker>>> = OnceLock::new();

fn get_workers() -> &'static Mutex<HashMap<u64, ActiveWorker>> {
    WORKERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn format_dynamic_title(template: &str, hours: f64, games_count: usize) -> String {
    if !template.contains('{') {
        return template.to_string();
    }
    let now = Local::now();
    let hours_str = format!("{:.1}", hours);
    let hours_int = format!("{}", hours as u64);
    let games_str = format!("{}", games_count);
    let time_str = now.format("%H:%M").to_string();
    let date_str = now.format("%d.%m").to_string();

    template
        .replace("{hours}", &hours_str)
        .replace("{hours_int}", &hours_int)
        .replace("{games}", &games_str)
        .replace("{games_count}", &games_str)
        .replace("{time}", &time_str)
        .replace("{date}", &date_str)
}

pub fn update_presence(
    account_id: u64,
    games: Vec<GameEntry>,
    custom_title: String,
    persona_state: PersonaState,
    afk_reply_custom: Option<String>,
) {
    let workers = get_workers().lock().unwrap();
    if let Some(worker) = workers.get(&account_id) {
        let _ = worker.cmd_tx.send(BoostCommand::UpdatePresence {
            games,
            custom_title,
            persona_state,
            afk_reply_custom,
        });
    }
}

pub fn start_boost(account: Arc<Mutex<Account>>) {
    let (id, username, password, _refresh_token, shared_secret, games, custom_title, persona_state, afk_reply_custom, proxy_url) = {
        let a = account.lock().unwrap();
        (
            a.id,
            a.username.clone(),
            a.password.clone(),
            a.refresh_token.clone().filter(|t| !t.trim().is_empty()),
            a.shared_secret.clone().filter(|s| !s.trim().is_empty()),
            a.games.clone(),
            a.custom_title.clone(),
            a.persona_state,
            a.afk_reply_custom.clone(),
            a.proxy.clone(),
        )
    };

    if username.trim().is_empty() {
        let mut a = account.lock().unwrap();
        a.status = AccountStatus::Error("Kullanıcı adı boş!".to_string());
        return;
    }

    {
        let mut a = account.lock().unwrap();
        a.status = AccountStatus::Connecting;
    }

    logger::log_info(&format!("[{}] Starting boost...", username));
    debug_log::log_packet(&username, PacketDirection::Outbound, "Connect", "Initiating Steam server connection", PacketCategory::Session);

    let (stop_tx, mut stop_rx) = watch::channel(false);
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<BoostCommand>();
    let (gui_tx, worker_rx) = tokio::io::duplex(256);
    let (worker_tx, mut gui_rx) = tokio::io::duplex(256);

    {
        let mut workers = get_workers().lock().unwrap();
        workers.insert(
            id,
            ActiveWorker {
                guard_tx: Some(gui_tx),
                stop_tx,
                cmd_tx,
            },
        );
    }

    let acc_for_task = account.clone();

    get_runtime().spawn(async move {
        let mut current_games = games;
        let mut current_title = custom_title;
        let mut current_persona = persona_state;
        let mut current_afk_custom = afk_reply_custom;
        let mut afk_cooldowns: HashMap<u64, tokio::time::Instant> = HashMap::new();
        let mut title_rotation_index: usize = 0;
        let mut rotation_ticks: u32 = 0;
        let mut inventory_ticks: u32 = 0;

        // Arka planda Steam Guard dinle
        let acc_guard_listener = acc_for_task.clone();
        let guard_user = username.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 256];
            if let Ok(n) = gui_rx.read(&mut buf).await {
                if n > 0 {
                    let prompt = String::from_utf8_lossy(&buf[..n]);
                    logger::log_warn(&format!("[{}] Steam Guard code or mobile confirmation required", guard_user));
                    let mut a = acc_guard_listener.lock().unwrap();
                    if matches!(a.status, AccountStatus::Connecting) {
                        a.status = AccountStatus::AwaitingSteamGuard {
                            hint: if prompt.contains("code") || prompt.contains("Code") {
                                "Enter the 5-digit Steam Guard code from email or Steam Mobile app.".to_string()
                            } else {
                                "Steam Guard confirmation pending (confirm on Steam Mobile or enter code).".to_string()
                            },
                        };
                    }
                }
            }
        });

        let mut worker_rx_opt = Some(worker_rx);
        let mut worker_tx_opt = Some(worker_tx);

        // Üstel Geri Çekilme (Exponential Backoff) ile bağlantı ve yeniden bağlanma döngüsü
        let mut reconnect_delay = 10u64;
        let mut reconnect_attempt = 0u32;

        'reconnect_loop: loop {
            // Sunucuları keşfet
            let server_list = match ServerList::discover().await {
                Ok(sl) => sl,
                Err(e) => {
                    logger::log_error(&format!("[{}] Unable to reach Steam servers: {}", username, e));
                    debug_log::log_packet(&username, PacketDirection::Inbound, "ServerList::discover", &format!("Error: {}", e), PacketCategory::Session);
                    {
                        let mut a = acc_for_task.lock().unwrap();
                        a.status = AccountStatus::Reconnecting {
                            attempt: reconnect_attempt + 1,
                            retry_in_secs: reconnect_delay,
                        };
                    }
                    tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
                    reconnect_delay = (reconnect_delay * 2).min(60);
                    reconnect_attempt += 1;
                    continue 'reconnect_loop;
                }
            };

            // Bağlantı kur
            let token_to_use = {
                let a = acc_for_task.lock().unwrap();
                a.refresh_token.clone()
            };

            let get_handler_streams = |rx_opt: &mut Option<tokio::io::DuplexStream>, tx_opt: &mut Option<tokio::io::DuplexStream>, id: u64, acc_ref: Arc<Mutex<Account>>| {
                if rx_opt.is_some() && tx_opt.is_some() {
                    (rx_opt.take().unwrap(), tx_opt.take().unwrap())
                } else {
                    let (new_gui_tx, new_worker_rx) = tokio::io::duplex(256);
                    let (new_worker_tx, mut new_gui_rx) = tokio::io::duplex(256);
                    if let Some(w) = get_workers().lock().unwrap().get_mut(&id) {
                        w.guard_tx = Some(new_gui_tx);
                    }
                    let acc_g = acc_ref.clone();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 256];
                        if let Ok(n) = new_gui_rx.read(&mut buf).await {
                            if n > 0 {
                                let mut a = acc_g.lock().unwrap();
                                if matches!(a.status, AccountStatus::Connecting) {
                                    a.status = AccountStatus::AwaitingSteamGuard {
                                        hint: "Steam Guard kodunu girin veya mobil uygulamadan onaylayın.".into(),
                                    };
                                }
                            }
                        }
                    });
                    (new_worker_rx, new_worker_tx)
                }
            };

            let connection_res = if let Some(ref token) = token_to_use {
                match Connection::access(&server_list, &username, token).await {
                    Ok(conn) => Ok(conn),
                    Err(err) => {
                        if !password.is_empty() {
                            let (rx, tx) = get_handler_streams(&mut worker_rx_opt, &mut worker_tx_opt, id, acc_for_task.clone());
                            let handler = UserProvidedAuthConfirmationHandler::new(rx, tx);
                            Connection::login(
                                &server_list,
                                &username,
                                &password,
                                FileGuardDataStore::user_cache(),
                                handler.or(DeviceConfirmationHandler),
                            )
                            .await
                        } else {
                            Err(err)
                        }
                    }
                }
            } else if let Some(ref sec) = shared_secret {
                Connection::login(
                    &server_list,
                    &username,
                    &password,
                    FileGuardDataStore::user_cache(),
                    SharedSecretAuthConfirmationHandler::new(sec),
                )
                .await
            } else {
                let (rx, tx) = get_handler_streams(&mut worker_rx_opt, &mut worker_tx_opt, id, acc_for_task.clone());
                let handler = UserProvidedAuthConfirmationHandler::new(rx, tx);
                Connection::login(
                    &server_list,
                    &username,
                    &password,
                    FileGuardDataStore::user_cache(),
                    handler.or(DeviceConfirmationHandler),
                )
                .await
            };

            let connection = match connection_res {
                Ok(conn) => {
                    reconnect_delay = 10;
                    reconnect_attempt = 0;
                    conn
                }
                Err(e) => {
                    let err_str = e.to_string();
                    logger::log_error(&format!("[{}] Login error: {}", username, err_str));
                    debug_log::log_packet(&username, PacketDirection::Inbound, "Login::Error", &err_str, PacketCategory::Session);
                    let friendly_err = if err_str.contains("InvalidCredentials") || err_str.contains("AccessToken") {
                        "Session key invalid or expired. Please log in via QR code again.".to_string()
                    } else if err_str.contains("InvalidPassword") {
                        "Invalid password entered!".to_string()
                    } else {
                        format!("Login failed: {}", e)
                    };
                    let mut a = acc_for_task.lock().unwrap();
                    a.status = AccountStatus::Error(friendly_err);
                    return;
                }
            };

            // Giriş başarılı, token'ı kaydet
            {
                let mut a = acc_for_task.lock().unwrap();
                a.status = AccountStatus::Boosting;
                if let Some(token) = connection.access_token() {
                    a.refresh_token = Some(token.to_string());
                }
            }
            logger::log_success(&format!("[{}] Steam connection established! Hour boosting active.", username));
            debug_log::log_packet(&username, PacketDirection::Inbound, "Login::Success", "Steam session opened", PacketCategory::Session);

            let my_steam_id = u64::from(connection.steam_id());

            // Ücretsiz lisansları talep et (Oyunların kütüphanede lisansının olduğundan emin olmak için)
            let free_appids: Vec<u32> = current_games.iter().map(|g| g.app_id).collect();
            if !free_appids.is_empty() {
                let claimed = crate::free_games::claim_free_licenses(&connection, &username, free_appids).await;
                if !claimed.is_empty() {
                    debug_log::log_packet(&username, PacketDirection::Outbound, "RequestFreeLicense", &format!("{:?}", claimed), PacketCategory::License);
                }
            }

            // Durum ve Oyun Paketlerini Oluşturma Fonksiyonu
            let build_presence = |games: &[GameEntry], custom_title: &str, persona: PersonaState, hours: f64| {
                let mut status_msg = CMsgClientChangeStatus::new();
                status_msg.persona_state = Some(persona.to_u32());
                status_msg.persona_state_flags = Some(
                    EPersonaStateFlag::k_EPersonaStateFlag_ClientTypeMobile as u32
                        | EPersonaStateFlag::k_EPersonaStateFlag_HasRichPresence as u32,
                );

                let mut games_msg = CMsgClientGamesPlayed::new();
                games_msg.client_os_type = Some(20); // Windows 10 64-bit

                let dyn_title = format_dynamic_title(custom_title.trim(), hours, games.len());

                // Çoklu Oyun Kasma (Steam max 32 eşzamanlı oyun destekler)
                if !dyn_title.is_empty() {
                    let mut custom_entry = GamePlayed::new();
                    custom_entry.set_game_id(15190414816125648896);
                    custom_entry.set_game_extra_info(dyn_title.clone());
                    games_msg.games_played.push(custom_entry);
                }

                for g in games {
                    let mut entry = GamePlayed::new();
                    entry.set_game_id(g.app_id as u64);
                    games_msg.games_played.push(entry);
                }

                (status_msg, games_msg)
            };

            let hours_now = { acc_for_task.lock().unwrap().hours_boosted };
            let (mut status_msg, mut games_msg) = build_presence(&current_games, &current_title, current_persona, hours_now);

            // İlk paketleri gönder
            let _ = connection.send(status_msg.clone()).await;
            debug_log::log_packet(&username, PacketDirection::Outbound, "CMsgClientChangeStatus", &format!("Persona: {:?}", current_persona), PacketCategory::Presence);

            if let Err(e) = connection.send(games_msg.clone()).await {
                logger::log_error(&format!("[{}] Failed to send games played packet: {}", username, e));
            } else {
                logger::log_info(&format!(
                    "[{}] Games played packet sent ({} games active). Custom Title: '{}'",
                    username, current_games.len(), current_title
                ));
                debug_log::log_packet(
                    &username,
                    PacketDirection::Outbound,
                    "CMsgClientGamesPlayed",
                    &format!("{} games | Title: '{}'", current_games.len(), current_title),
                    PacketCategory::Presence,
                );
            }

            // Modern Steam Chat oturumunu aktifleştir
            let mut req_sessions = CFriendsMessages_GetActiveMessageSessions_Request::new();
            req_sessions.set_lastmessage_since(0);
            req_sessions.set_only_sessions_with_messages(false);
            if let Err(e) = connection.service_method(req_sessions).await {
                logger::log_warn(&format!("[{}] Chat session init: {}", username, e));
            } else {
                logger::log_info(&format!("[{}] Modern Steam Chat session activated.", username));
                debug_log::log_packet(&username, PacketDirection::Outbound, "FriendMessages.GetActiveMessageSessions#1", "Chat session opened", PacketCategory::Chat);
            }

            // İlk envanter kontrolü
            let inv_user = username.clone();
            let inv_proxy = proxy_url.clone();
            tokio::spawn(async move {
                crate::inventory::check_inventory_tracker(inv_user, my_steam_id, inv_proxy).await;
            });

            // Akışlar
            let mut session_stream = connection.on::<CMsgClientPlayingSessionState>();
            let mut modern_chat_stream = connection.on_notification::<CFriendMessages_IncomingMessage_Notification>();
            let mut chat_room_stream = connection.on_notification::<CChatRoom_IncomingChatMessage_Notification>();
            let mut legacy_friend_stream = connection.on::<CMsgClientFriendMsgIncoming>();
            let mut item_announcement_stream = connection.on::<CMsgClientItemAnnouncements>();
            let mut is_paused_for_game = false;
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            let reply_to_friend = |conn: Connection, user: String, target_id: u64, text: String| {
                tokio::spawn(async move {
                    debug_log::log_packet(
                        &user,
                        PacketDirection::Outbound,
                        "FriendMessages.SendMessage#1",
                        &format!("Target: {} | Reply: \"{}\"", target_id, text),
                        PacketCategory::Chat,
                    );

                    let mut modern_req = CFriendMessages_SendMessage_Request::new();
                    modern_req.set_steamid(target_id);
                    modern_req.set_chat_entry_type(1);
                    modern_req.set_message(text.clone());
                    modern_req.set_contains_bbcode(false);
                    modern_req.set_echo_to_sender(true);

                    let modern_res = conn.service_method(modern_req).await;

                    let mut legacy_reply = CMsgClientFriendMsg::new();
                    legacy_reply.set_steamid(target_id);
                    legacy_reply.set_chat_entry_type(1);
                    legacy_reply.set_message(text.as_bytes().to_vec());
                    let legacy_res = conn.send(legacy_reply).await;

                    if modern_res.is_ok() || legacy_res.is_ok() {
                        logger::log_success(&format!(
                            "💬 [AFK REPLIED] [{}] -> Friend ({}) | Message: \"{}\"",
                            user, target_id, text
                        ));
                    } else {
                        logger::log_warn(&format!(
                            "[{}] Failed to deliver AFK reply. Modern: {:?}, Legacy: {:?}",
                            user,
                            modern_res.err(),
                            legacy_res.err()
                        ));
                    }
                });
            };

            loop {
                tokio::select! {
                    // 1. Gerçek oyun oturum çakışması algılama (Auto-Pause & Auto-Resume)
                    Some(Ok(session_state)) = session_stream.next() => {
                        let is_blocked = session_state.playing_blocked.unwrap_or(false);
                        let playing_app = session_state.playing_app.unwrap_or(0);
                        debug_log::log_packet(
                            &username,
                            PacketDirection::Inbound,
                            "CMsgClientPlayingSessionState",
                            &format!("Blocked: {}, App: {}", is_blocked, playing_app),
                            PacketCategory::Session,
                        );

                        if is_blocked && !is_paused_for_game {
                            is_paused_for_game = true;
                            logger::log_warn(&format!("[{}] 🎮 Real game detected (AppID: {}). Boosting automatically paused!", username, playing_app));
                            let empty_msg = CMsgClientGamesPlayed::new();
                            let _ = connection.send(empty_msg).await;
                            let mut a = acc_for_task.lock().unwrap();
                            a.status = AccountStatus::PausedForGame {
                                app_id: playing_app,
                                app_name: format!("AppID {}", playing_app),
                            };
                            crate::telegram::send_telegram_alert(&format!(
                                "🎮 *[Real Game Detected]* [{}]\nGame launched on computer (AppID {}). Hour boosting automatically paused.",
                                username, playing_app
                            ));
                        } else if !is_blocked && is_paused_for_game {
                            is_paused_for_game = false;
                            logger::log_success(&format!("[{}] 🎮 Real game closed. Resuming hour boosting!", username));
                            let hours = { acc_for_task.lock().unwrap().hours_boosted };
                            let (s_msg, g_msg) = build_presence(&current_games, &current_title, current_persona, hours);
                            status_msg = s_msg;
                            games_msg = g_msg;
                            let _ = connection.send(status_msg.clone()).await;
                            let _ = connection.send(games_msg.clone()).await;
                            let mut a = acc_for_task.lock().unwrap();
                            a.status = AccountStatus::Boosting;
                            crate::telegram::send_telegram_alert(&format!(
                                "▶️ *[Resuming Boosting]* [{}]\nReal game closed. Hour boosting resumed automatically.",
                                username
                            ));
                        }
                    }

                    // 2. Canlı Hot-Reload (Arayüzden ayar/oyun/başlık değiştiğinde kesintisiz güncelleme)
                    Some(cmd) = cmd_rx.recv() => {
                        match cmd {
                            BoostCommand::UpdatePresence { games, custom_title, persona_state, afk_reply_custom } => {
                                current_games = games;
                                current_title = custom_title;
                                current_persona = persona_state;
                                current_afk_custom = afk_reply_custom;
                                logger::log_info(&format!("[{}] Live settings updated (Hot-Reload). Sending presence to Steam...", username));
                                let hours = { acc_for_task.lock().unwrap().hours_boosted };
                                let (s_msg, g_msg) = build_presence(&current_games, &current_title, current_persona, hours);
                                status_msg = s_msg;
                                games_msg = g_msg;
                                if !is_paused_for_game {
                                    let _ = connection.send(status_msg.clone()).await;
                                    let _ = connection.send(games_msg.clone()).await;
                                }
                            }
                            BoostCommand::Stop => {
                                break;
                            }
                        }
                    }

                    // 3. Modern Steam Chat Arkadaş Mesajı
                    Some(Ok(msg_notif)) = modern_chat_stream.next() => {
                        let from_id = msg_notif.steamid_friend();
                        let entry_type = msg_notif.chat_entry_type.unwrap_or(1);
                        let is_echo = msg_notif.local_echo.unwrap_or(false);
                        let text = msg_notif.message.as_deref().unwrap_or("");

                        debug_log::log_packet(
                            &username,
                            PacketDirection::Inbound,
                            "FriendMessagesClient.IncomingMessage#1",
                            &format!("From: {} | Message: \"{}\"", from_id, text),
                            PacketCategory::Chat,
                        );

                        logger::log_info(&format!("[{}] 📩 Chat message received (ID: {}): \"{}\"", username, from_id, text));

                        if from_id > 0 && from_id != my_steam_id && !is_echo && (entry_type == 1 || entry_type == 0) {
                            let global_settings = crate::data::get_global_settings();
                            let is_afk_active = global_settings.afk_auto_reply;

                            if is_afk_active {
                                let cooldown_dur = Duration::from_secs((global_settings.afk_cooldown_mins.max(1) as u64) * 60);
                                let now = tokio::time::Instant::now();
                                let should_reply = match afk_cooldowns.get(&from_id) {
                                    Some(last_time) => now.duration_since(*last_time) >= cooldown_dur,
                                    None => true,
                                };

                                if should_reply {
                                    afk_cooldowns.insert(from_id, now);
                                    let reply_text = current_afk_custom
                                        .as_ref()
                                        .filter(|s| !s.trim().is_empty())
                                        .cloned()
                                        .unwrap_or_else(|| global_settings.afk_reply_message.clone());

                                    reply_to_friend(connection.clone(), username.clone(), from_id, reply_text.clone());

                                    if global_settings.telegram_notify_afk {
                                        crate::telegram::send_telegram_alert(&format!(
                                            "💬 *[AFK Auto-Reply]* [{}]\n👤 Friend ID: `{}`\n📩 Incoming: \"{}\"\n📤 Reply: \"{}\"",
                                            username, from_id, text, reply_text
                                        ));
                                    }
                                } else {
                                    logger::log_info(&format!("[{}] Cooldown active ({} min) for friend ({}).", username, global_settings.afk_cooldown_mins, from_id));
                                }
                            }
                        }
                    }

                    // 4. Steam Chat Room / Grup Mesajı
                    Some(Ok(chat_notif)) = chat_room_stream.next() => {
                        let from_id = chat_notif.steamid_sender.unwrap_or(0);
                        let text = chat_notif.message.as_deref().unwrap_or("");

                        debug_log::log_packet(
                            &username,
                            PacketDirection::Inbound,
                            "ChatRoomClient.NotifyIncomingChatMessage#1",
                            &format!("Gönderen: {} | Mesaj: \"{}\"", from_id, text),
                            PacketCategory::Chat,
                        );

                        if from_id > 0 && from_id != my_steam_id {
                            let global_settings = crate::data::get_global_settings();
                            let is_afk_active = global_settings.afk_auto_reply;

                            if is_afk_active {
                                let cooldown_dur = Duration::from_secs((global_settings.afk_cooldown_mins.max(1) as u64) * 60);
                                let now = tokio::time::Instant::now();
                                let should_reply = match afk_cooldowns.get(&from_id) {
                                    Some(last_time) => now.duration_since(*last_time) >= cooldown_dur,
                                    None => true,
                                };

                                if should_reply {
                                    afk_cooldowns.insert(from_id, now);
                                    let reply_text = current_afk_custom
                                        .as_ref()
                                        .filter(|s| !s.trim().is_empty())
                                        .cloned()
                                        .unwrap_or_else(|| global_settings.afk_reply_message.clone());

                                    reply_to_friend(connection.clone(), username.clone(), from_id, reply_text);
                                }
                            }
                        }
                    }

                    // 5. Eski Steam Protokolü Arkadaş Mesajı (Legacy Fallback)
                    Some(Ok(friend_msg)) = legacy_friend_stream.next() => {
                        let from_id = friend_msg.steamid_from();
                        let entry_type = friend_msg.chat_entry_type.unwrap_or(0);
                        let msg_bytes = friend_msg.message.as_deref().unwrap_or(&[]);
                        let text = String::from_utf8_lossy(msg_bytes);

                        debug_log::log_packet(
                            &username,
                            PacketDirection::Inbound,
                            "CMsgClientFriendMsgIncoming",
                            &format!("Gönderen: {} | Mesaj: \"{}\"", from_id, text),
                            PacketCategory::Chat,
                        );

                        if from_id > 0 && from_id != my_steam_id && entry_type == 1 {
                            let global_settings = crate::data::get_global_settings();
                            let is_afk_active = global_settings.afk_auto_reply;

                            if is_afk_active {
                                let cooldown_dur = Duration::from_secs((global_settings.afk_cooldown_mins.max(1) as u64) * 60);
                                let now = tokio::time::Instant::now();
                                let should_reply = match afk_cooldowns.get(&from_id) {
                                    Some(last_time) => now.duration_since(*last_time) >= cooldown_dur,
                                    None => true,
                                };

                                if should_reply {
                                    afk_cooldowns.insert(from_id, now);
                                    let reply_text = current_afk_custom
                                        .as_ref()
                                        .filter(|s| !s.trim().is_empty())
                                        .cloned()
                                        .unwrap_or_else(|| global_settings.afk_reply_message.clone());

                                    reply_to_friend(connection.clone(), username.clone(), from_id, reply_text);
                                }
                            }
                        }
                    }

                    // 6. Steam Koleksiyon Kartı Düşürme Bildirimi (Item Drop & Market Value Tracker)
                    Some(Ok(item_announcement)) = item_announcement_stream.next() => {
                        let global_settings = crate::data::get_global_settings();
                        if global_settings.card_drop_tracking {
                            let items = item_announcement.unseen_items.clone();
                            let user_name = username.clone();
                            let games_clone = current_games.clone();

                            tokio::spawn(async move {
                                for item in items {
                                    let source_app = if item.source_appid.unwrap_or(0) > 0 {
                                        item.source_appid.unwrap_or(0)
                                    } else {
                                        item.appid.unwrap_or(0)
                                    };
                                    let asset_id = item.asset_id;
                                    process_card_drop(user_name.clone(), my_steam_id, source_app, asset_id, games_clone.clone()).await;
                                }
                            });
                        }
                    }

                    // 7. 60 saniyelik Keepalive, Akıllı Başlık Döngüsü ve Saat Sayacı
                    _ = interval.tick() => {
                        if !is_paused_for_game {
                            let hours = {
                                let mut a = acc_for_task.lock().unwrap();
                                if !matches!(a.status, AccountStatus::Boosting) {
                                    break;
                                }
                                a.hours_boosted += 1.0 / 60.0;
                                a.hours_boosted
                            };

                            let global_settings = crate::data::get_global_settings();

                            // Dinamik Başlık Döngüsü (Smart Status Scenarios)
                            rotation_ticks += 1;
                            if global_settings.title_rotation && !global_settings.title_rotation_list.is_empty() {
                                let interval_ticks = (global_settings.title_rotation_interval_secs / 60).max(1);
                                if rotation_ticks >= interval_ticks {
                                    rotation_ticks = 0;
                                    title_rotation_index = (title_rotation_index + 1) % global_settings.title_rotation_list.len();
                                    current_title = global_settings.title_rotation_list[title_rotation_index].clone();
                                    logger::log_info(&format!("[{}] 🔄 Dynamic title rotated: \"{}\"", username, current_title));
                                }
                            }

                            // Envanter Takibi (Her 15 dakikada bir kontrol)
                            inventory_ticks += 1;
                            if global_settings.inventory_tracking && inventory_ticks >= (global_settings.auto_claim_interval_mins.min(15)) {
                                inventory_ticks = 0;
                                let inv_u = username.clone();
                                let inv_p = proxy_url.clone();
                                tokio::spawn(async move {
                                    crate::inventory::check_inventory_tracker(inv_u, my_steam_id, inv_p).await;
                                });
                            }

                            // Dinamik başlık paketlerini güncelle
                            let (s_msg, g_msg) = build_presence(&current_games, &current_title, current_persona, hours);
                            status_msg = s_msg;
                            games_msg = g_msg;

                            // Keepalive sinyalleri
                            if let Err(e) = connection.send(status_msg.clone()).await {
                                logger::log_warn(&format!("[{}] Failed to send keepalive status: {}", username, e));
                            }
                            if let Err(e) = connection.send(games_msg.clone()).await {
                                logger::log_warn(&format!("[{}] Connection lost (Tuesday maintenance or network issue): {}. Reconnecting...", username, e));
                                break; // Reconnect döngüsüne düş
                            }
                        }
                    }

                    // 8. Durdurma sinyali
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            logger::log_info(&format!("[{}] Hour boosting stopped by user.", username));
                            let clear_msg = CMsgClientGamesPlayed::new();
                            let _ = connection.send(clear_msg).await;
                            let mut a = acc_for_task.lock().unwrap();
                            a.status = AccountStatus::Idle;
                            return;
                        }
                    }
                }
            }

            // Yeniden bağlanma
            if *stop_rx.borrow() {
                break;
            }
            logger::log_warn(&format!("[{}] Connection lost. Reconnecting in {}s (Attempt: {})...", username, reconnect_delay, reconnect_attempt + 1));
            {
                let mut a = acc_for_task.lock().unwrap();
                a.status = AccountStatus::Reconnecting {
                    attempt: reconnect_attempt + 1,
                    retry_in_secs: reconnect_delay,
                };
            }
            tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
            reconnect_delay = (reconnect_delay * 2).min(60);
            reconnect_attempt += 1;
        }

        {
            let mut a = acc_for_task.lock().unwrap();
            if matches!(a.status, AccountStatus::Boosting | AccountStatus::Connecting | AccountStatus::AwaitingSteamGuard { .. } | AccountStatus::PausedForGame { .. } | AccountStatus::Reconnecting { .. }) {
                a.status = AccountStatus::Idle;
            }
        }
    });
}

pub fn submit_guard_code(account_id: u64, code: &str) {
    let clean = format!("{}\n", code.trim());
    let mut guard_stream = {
        let mut workers = get_workers().lock().unwrap();
        workers.get_mut(&account_id).and_then(|w| w.guard_tx.take())
    };

    if let Some(mut stream) = guard_stream.take() {
        get_runtime().spawn(async move {
            let _ = stream.write_all(clean.as_bytes()).await;
        });
    }
}

pub fn stop_boost(account_id: u64, account: Arc<Mutex<Account>>) {
    {
        let workers = get_workers().lock().unwrap();
        if let Some(worker) = workers.get(&account_id) {
            let _ = worker.stop_tx.send(true);
            let _ = worker.cmd_tx.send(BoostCommand::Stop);
        }
    }
    {
        let mut a = account.lock().unwrap();
        a.status = AccountStatus::Idle;
    }
}

pub fn restart_boost(account_id: u64, account: Arc<Mutex<Account>>) {
    stop_boost(account_id, account.clone());
    get_runtime().spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        start_boost(account);
    });
}

async fn process_card_drop(
    username: String,
    my_steam_id: u64,
    source_appid: u32,
    asset_id: Option<u64>,
    games: Vec<GameEntry>,
) {
    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => reqwest::Client::new(),
    };

    // 1. Oyun Adını Belirle
    let mut game_name = games
        .iter()
        .find(|g| g.app_id == source_appid)
        .map(|g| g.name.clone());

    if game_name.is_none() && source_appid > 0 {
        let store_url = format!("https://store.steampowered.com/api/appdetails?appids={}", source_appid);
        if let Ok(resp) = client.get(&store_url).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(name) = json.get(&source_appid.to_string())
                    .and_then(|v| v.get("data"))
                    .and_then(|d| d.get("name"))
                    .and_then(|n| n.as_str())
                {
                    game_name = Some(name.to_string());
                }
            }
        }
    }
    let game_name = game_name.unwrap_or_else(|| {
        if source_appid > 0 {
            format!("AppID {}", source_appid)
        } else {
            "Bilinmeyen Oyun".to_string()
        }
    });

    let mut card_name: Option<String> = None;
    let mut price_str: Option<String> = None;

    // 2. Kullanıcı Envanterinden sorgulamayı dene (asset_id mevcutsa)
    if let Some(aid) = asset_id {
        if my_steam_id > 0 {
            let inv_url = format!("https://steamcommunity.com/inventory/{}/753/6?l=turkish&count=50", my_steam_id);
            if let Ok(resp) = client.get(&inv_url).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let aid_str = aid.to_string();
                    let class_id = json.get("assets")
                        .and_then(|a| a.as_array())
                        .and_then(|arr| {
                            arr.iter().find(|item| {
                                item.get("assetid").and_then(|id| id.as_str()) == Some(&aid_str)
                            })
                        })
                        .and_then(|item| item.get("classid").and_then(|cid| cid.as_str()));

                    if let Some(cid) = class_id {
                        if let Some(descriptions) = json.get("descriptions").and_then(|d| d.as_array()) {
                            if let Some(desc) = descriptions.iter().find(|d| {
                                d.get("classid").and_then(|c| c.as_str()) == Some(cid)
                            }) {
                                if let Some(n) = desc.get("name").and_then(|n| n.as_str()) {
                                    card_name = Some(n.to_string());
                                }
                                if let Some(hash) = desc.get("market_hash_name").and_then(|h| h.as_str()) {
                                    if let Ok(price_url) = reqwest::Url::parse_with_params(
                                        "https://steamcommunity.com/market/priceoverview/",
                                        &[("appid", "753"), ("currency", "1"), ("market_hash_name", hash)],
                                    ) {
                                        if let Ok(p_resp) = client.get(price_url).send().await {
                                            if let Ok(p_json) = p_resp.json::<serde_json::Value>().await {
                                                if let Some(lowest) = p_json.get("lowest_price").and_then(|p| p.as_str()) {
                                                    price_str = Some(lowest.to_string());
                                                } else if let Some(med) = p_json.get("median_price").and_then(|p| p.as_str()) {
                                                    price_str = Some(med.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Envanterden tespit edilemediyse Steam Pazarı aramasından kart ve pazar fiyatını al
    if (card_name.is_none() || price_str.is_none()) && source_appid > 0 {
        let market_url = format!(
            "https://steamcommunity.com/market/search/render/?query=&start=0&count=1&search_descriptions=0&sort_column=popular&sort_dir=desc&appid=753&category_753_Game%5B%5D=tag_app_{}&category_753_item_class%5B%5D=tag_item_class_2&norender=1",
            source_appid
        );
        if let Ok(resp) = client.get(&market_url).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(results) = json.get("results").and_then(|r| r.as_array()) {
                    if let Some(first) = results.first() {
                        if card_name.is_none() {
                            if let Some(n) = first.get("name").and_then(|n| n.as_str()) {
                                card_name = Some(n.to_string());
                            }
                        }
                        if price_str.is_none() {
                            if let Some(p) = first.get("sell_price_text").and_then(|p| p.as_str()) {
                                price_str = Some(p.to_string());
                            } else if let Some(p) = first.get("sale_price_text").and_then(|p| p.as_str()) {
                                price_str = Some(p.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let final_card = card_name.unwrap_or_else(|| "Trading Card".to_string());
    let final_price = price_str.unwrap_or_else(|| "Not listed on market yet / Unknown".to_string());

    crate::inventory::record_card_drop(crate::inventory::CardDropEvent {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        username: username.clone(),
        game_name: game_name.clone(),
        card_name: final_card.clone(),
        market_price: final_price.clone(),
    });

    logger::log_success(&format!(
        "🃏 [CARD DROPPED!] [{}] 🎮 Game: {} | 💎 Card: {} | 💰 Steam Market Price: {}",
        username, game_name, final_card, final_price
    ));

    debug_log::log_packet(
        &username,
        PacketDirection::Inbound,
        "CMsgClientItemAnnouncements",
        &format!("Card: {} ({}) | Price: {}", final_card, game_name, final_price),
        PacketCategory::Economy,
    );

    let global_settings = crate::data::get_global_settings();
    if global_settings.telegram_notify_cards {
        crate::telegram::send_telegram_alert(&format!(
            "🃏 *[Trading Card Dropped!]* [{}]\n🎮 *Game:* {}\n💎 *Card:* {}\n💰 *Market Price:* {}",
            username, game_name, final_card, final_price
        ));
    }
}
