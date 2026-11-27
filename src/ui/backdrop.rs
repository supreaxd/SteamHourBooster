use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime};
use eframe::egui::{
    self,
    epaint::{Mesh, Vertex},
    Color32, ColorImage, Context, LayerId, Pos2, Rect, Shape, TextureHandle, TextureOptions,
};
use crate::data::CustomThemeColors;

#[derive(Clone, Copy, PartialEq, Eq)]
struct PhysRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl PhysRect {
    fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }
    fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }
    fn contains(self, other: PhysRect) -> bool {
        other.left >= self.left
            && other.top >= self.top
            && other.right <= self.right
            && other.bottom <= self.bottom
    }
    fn intersects(self, other: PhysRect) -> Option<PhysRect> {
        let l = self.left.max(other.left);
        let t = self.top.max(other.top);
        let r = self.right.min(other.right);
        let b = self.bottom.min(other.bottom);
        if r > l && b > t {
            Some(PhysRect { left: l, top: t, right: r, bottom: b })
        } else {
            None
        }
    }
    fn union(self, other: PhysRect) -> PhysRect {
        PhysRect {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

struct BlurredFrame {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
    scene_rect: PhysRect,
    seq: u64,
}

struct SharedCapture {
    our_hwnd: isize,
    client_rect: Option<PhysRect>,
    active: bool,
    blur_level: f32,
    blur_key: i32,
    ctx: Option<Context>,
    latest: Option<BlurredFrame>,
    seq_counter: u64,
}

/// High-performance Liquid Glass backdrop:
/// 1. Captures the entire active monitor (or union if straddling monitors) via Z-order `PrintWindow` + desktop/icons
///    on a background worker thread, reusing buffers so dragging across a monitor runs at native FPS with zero jitter.
/// 2. Runs the 3-pass Gaussian blur *off* the UI thread so egui stays buttery smooth.
/// 3. Falls back safely to guarded single-monitor `Magnification` only if an opaque unredirected game window
///    (e.g. borderless DX11/Vulkan) fails `PrintWindow`, and never uses `Magnification` while straddling monitors.
pub struct DesktopBackdrop {
    shared: Arc<Mutex<SharedCapture>>,
    stop_flag: Arc<AtomicBool>,
    worker_started: bool,

    last_uploaded_seq: u64,
    last_scene_rect: Option<PhysRect>,
    texture: Option<TextureHandle>,
}

impl Drop for DesktopBackdrop {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

impl DesktopBackdrop {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(SharedCapture {
                our_hwnd: 0,
                client_rect: None,
                active: false,
                blur_level: 6.0,
                blur_key: 120,
                ctx: None,
                latest: None,
                seq_counter: 0,
            })),
            stop_flag: Arc::new(AtomicBool::new(false)),
            worker_started: false,
            last_uploaded_seq: 0,
            last_scene_rect: None,
            texture: None,
        }
    }

    #[cfg(target_os = "windows")]
    fn ensure_worker_started(&mut self) {
        if self.worker_started {
            return;
        }
        self.worker_started = true;
        let shared = self.shared.clone();
        let stop = self.stop_flag.clone();
        let _ = std::thread::Builder::new()
            .name("liquid-glass-backdrop".to_string())
            .spawn(move || {
                backdrop_worker(shared, stop);
            });
    }

    pub fn draw(&mut self, ctx: &Context, hwnd_opt: Option<isize>, colors: &CustomThemeColors) {
        let blur_level = colors.blur_level.clamp(0.0, 10.0);
        let blur_active = blur_level > 0.05;
        let blur_key = (blur_level * 20.0).round() as i32;

        #[cfg(target_os = "windows")]
        let current_client_phys = hwnd_opt.and_then(get_window_client_rect_phys);
        #[cfg(not(target_os = "windows"))]
        let current_client_phys: Option<PhysRect> = None;

        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = hwnd_opt {
                if blur_active {
                    self.ensure_worker_started();
                }
                if let Ok(mut guard) = self.shared.lock() {
                    guard.our_hwnd = hwnd;
                    guard.client_rect = current_client_phys;
                    guard.active = blur_active;
                    guard.blur_level = blur_level;
                    guard.blur_key = blur_key;
                    if guard.ctx.is_none() {
                        guard.ctx = Some(ctx.clone());
                    }
                }
            }
        }

        if !blur_active {
            return;
        }

        // Upload newly blurred frame only when background worker produces a new sequence
        let upload_snapshot: Option<(usize, usize, PhysRect, u64, Vec<u8>)> = {
            if let Ok(guard) = self.shared.lock() {
                if let Some(ref bf) = guard.latest {
                    if bf.seq != self.last_uploaded_seq || self.texture.is_none() {
                        Some((bf.w, bf.h, bf.scene_rect, bf.seq, bf.rgba.clone()))
                    } else {
                        self.last_scene_rect = Some(bf.scene_rect);
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((w, h, scene_rect, seq, rgba)) = upload_snapshot {
            let color_image = ColorImage::from_rgba_unmultiplied([w, h], &rgba);
            match &mut self.texture {
                Some(handle) => handle.set(color_image, TextureOptions::LINEAR),
                None => {
                    self.texture = Some(ctx.load_texture(
                        "liquid_glass_backdrop",
                        color_image,
                        TextureOptions::LINEAR,
                    ));
                }
            }
            self.last_uploaded_seq = seq;
            self.last_scene_rect = Some(scene_rect);
        }

        let Some(tex) = &self.texture else {
            return;
        };

        let screen_rect = ctx.screen_rect();
        let uv_rect = match (current_client_phys, self.last_scene_rect) {
            (Some(cur), Some(scene)) if scene.width() > 0 && scene.height() > 0 => {
                let sw = scene.width() as f32;
                let sh = scene.height() as f32;
                let u0 = ((cur.left - scene.left) as f32 / sw).clamp(0.0, 0.99);
                let v0 = ((cur.top - scene.top) as f32 / sh).clamp(0.0, 0.99);
                let u1 = ((cur.right - scene.left) as f32 / sw).clamp(u0 + 0.01, 1.0);
                let v1 = ((cur.bottom - scene.top) as f32 / sh).clamp(v0 + 0.01, 1.0);
                Rect::from_min_max(Pos2::new(u0, v0), Pos2::new(u1, v1))
            }
            _ => Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        };

        let painter = ctx.layer_painter(LayerId::background());
        draw_refracted_glass_mesh(&painter, screen_rect, uv_rect, tex.id());
    }
}

/// Draws the backdrop texture using a 7x7 curved lens mesh so all 4 window edges
/// optically bend / curve light smoothly and equally along a 26px Liquid Glass bezel without UV inversion.
fn draw_refracted_glass_mesh(
    painter: &egui::Painter,
    rect: Rect,
    uv_rect: Rect,
    tex_id: egui::TextureId,
) {
    let bevel = 26.0_f32.min(rect.width() * 0.22).min(rect.height() * 0.22);
    if bevel < 5.0 {
        painter.image(tex_id, rect, uv_rect, Color32::WHITE);
        return;
    }

    let b_mid = bevel * 0.50;
    let b_outer = bevel * 0.22;
    let xs = [
        rect.left(),
        rect.left() + b_outer,
        rect.left() + b_mid,
        rect.left() + bevel,
        rect.right() - bevel,
        rect.right() - b_mid,
        rect.right() - b_outer,
        rect.right(),
    ];
    let ys = [
        rect.top(),
        rect.top() + b_outer,
        rect.top() + b_mid,
        rect.top() + bevel,
        rect.bottom() - bevel,
        rect.bottom() - b_mid,
        rect.bottom() - b_outer,
        rect.bottom(),
    ];

    let u_span = uv_rect.width();
    let v_span = uv_rect.height();

    let bx = (bevel / rect.width()).clamp(0.005, 0.25);
    let by = (bevel / rect.height()).clamp(0.005, 0.25);

    // Smooth monotonic S-curve optical refraction equal across all 4 edges:
    // center stays 1:1 flat while outer 26px rim curves & bends underlying background pixels cleanly.
    let warp_u_fracs = [
        bx * 0.22,
        bx * 0.36,
        bx * 0.62,
        bx,
        1.0 - bx,
        1.0 - bx * 0.62,
        1.0 - bx * 0.36,
        1.0 - bx * 0.22,
    ];
    let warp_v_fracs = [
        by * 0.22,
        by * 0.36,
        by * 0.62,
        by,
        1.0 - by,
        1.0 - by * 0.62,
        1.0 - by * 0.36,
        1.0 - by * 0.22,
    ];

    let mut mesh = Mesh::with_texture(tex_id);
    for iy in 0..8usize {
        for ix in 0..8usize {
            mesh.vertices.push(Vertex {
                pos: Pos2::new(xs[ix], ys[iy]),
                uv: Pos2::new(
                    uv_rect.min.x + warp_u_fracs[ix] * u_span,
                    uv_rect.min.y + warp_v_fracs[iy] * v_span,
                ),
                color: Color32::WHITE,
            });
        }
    }

    for iy in 0..7u32 {
        for ix in 0..7u32 {
            let i00 = iy * 8 + ix;
            let i10 = i00 + 1;
            let i01 = i00 + 8;
            let i11 = i01 + 1;
            mesh.add_triangle(i00, i10, i01);
            mesh.add_triangle(i10, i11, i01);
        }
    }

    painter.add(Shape::mesh(mesh));
}

// ─── Windows Compositor (PrintWindow + Wallpaper + Safe Single-Monitor Mag Fallback) ──

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy)]
struct WinPoint {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct MonitorInfo {
    cb_size: u32,
    rc_monitor: WinRect,
    rc_work: WinRect,
    dw_flags: u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct BitmapInfoHeader {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct BitmapInfo {
    bmi_header: BitmapInfoHeader,
    bmi_colors: [u32; 1],
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy)]
struct MagImageHeader {
    width: u32,
    height: u32,
    format: Guid,
    stride: u32,
    offset: u32,
    cb_size: usize,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct Msg {
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: WinPoint,
}

#[cfg(target_os = "windows")]
fn get_window_client_rect_phys(hwnd: isize) -> Option<PhysRect> {
    use std::ffi::c_void;
    extern "system" {
        fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    }
    unsafe {
        let user32 = LoadLibraryA(b"user32.dll\0".as_ptr());
        if user32.is_null() {
            return None;
        }
        let p_client_rect = GetProcAddress(user32, b"GetClientRect\0".as_ptr());
        let p_client_to_screen = GetProcAddress(user32, b"ClientToScreen\0".as_ptr());
        let p_is_iconic = GetProcAddress(user32, b"IsIconic\0".as_ptr());
        if p_client_rect.is_null() || p_client_to_screen.is_null() {
            return None;
        }
        if !p_is_iconic.is_null() {
            let is_iconic: unsafe extern "system" fn(isize) -> i32 = std::mem::transmute(p_is_iconic);
            if is_iconic(hwnd) != 0 {
                return None;
            }
        }
        let get_client_rect: unsafe extern "system" fn(isize, *mut WinRect) -> i32 =
            std::mem::transmute(p_client_rect);
        let client_to_screen: unsafe extern "system" fn(isize, *mut WinPoint) -> i32 =
            std::mem::transmute(p_client_to_screen);

        let mut cr = WinRect { left: 0, top: 0, right: 0, bottom: 0 };
        if get_client_rect(hwnd, &mut cr) == 0 {
            return None;
        }
        let w = cr.right - cr.left;
        let h = cr.bottom - cr.top;
        if w < 16 || h < 16 {
            return None;
        }
        let mut pt = WinPoint { x: 0, y: 0 };
        if client_to_screen(hwnd, &mut pt) == 0 {
            return None;
        }
        Some(PhysRect {
            left: pt.x,
            top: pt.y,
            right: pt.x + w,
            bottom: pt.y + h,
        })
    }
}

#[cfg(target_os = "windows")]
struct MagFallbackFrame {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

#[cfg(target_os = "windows")]
thread_local! {
    static TLS_MAG_FRAME: RefCell<Option<MagFallbackFrame>> = const { RefCell::new(None) };
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn mag_scaling_callback(
    _hwnd: isize,
    srcdata: *const u8,
    srcheader: MagImageHeader,
    _destdata: *mut u8,
    _destheader: MagImageHeader,
    _unclipped: WinRect,
    _clipped: WinRect,
    _dirty: isize,
) -> i32 {
    if srcdata.is_null() || srcheader.width < 8 || srcheader.height < 8 || srcheader.stride < srcheader.width * 4 {
        return 0;
    }

    let sw = srcheader.width as usize;
    let sh = srcheader.height as usize;
    let stride = srcheader.stride as usize;
    let offset = srcheader.offset as usize;
    let min_bytes = offset.saturating_add((sh - 1).saturating_mul(stride)).saturating_add(sw * 4);
    if srcheader.cb_size < min_bytes {
        return 0;
    }

    let is_bgra = srcheader.format.data1 == 0x6fddc324;

    // Target max ~768px wide for instant off-thread blur
    let step = ((sw / 768).max(2)).min(4);
    let hw = (sw / step).max(2);
    let hh = (sh / step).max(2);
    let mut out = vec![0u8; hw * hh * 4];
    let base_ptr = srcdata.add(offset);

    for y in 0..hh {
        let sy = (y * step).min(sh - 1);
        let sy2 = (sy + 1).min(sh - 1);
        let row0 = base_ptr.add(sy * stride);
        let row1 = base_ptr.add(sy2 * stride);
        for x in 0..hw {
            let sx = (x * step).min(sw - 1) * 4;
            let sx2 = (sx / 4 + 1).min(sw - 1) * 4;
            let p00 = row0.add(sx);
            let p10 = row0.add(sx2);
            let p01 = row1.add(sx);
            let p11 = row1.add(sx2);
            let (r, g, b) = if is_bgra {
                (
                    ((*p00.add(2) as u32 + *p10.add(2) as u32 + *p01.add(2) as u32 + *p11.add(2) as u32) >> 2) as u8,
                    ((*p00.add(1) as u32 + *p10.add(1) as u32 + *p01.add(1) as u32 + *p11.add(1) as u32) >> 2) as u8,
                    ((*p00 as u32 + *p10 as u32 + *p01 as u32 + *p11 as u32) >> 2) as u8,
                )
            } else {
                (
                    ((*p00 as u32 + *p10 as u32 + *p01 as u32 + *p11 as u32) >> 2) as u8,
                    ((*p00.add(1) as u32 + *p10.add(1) as u32 + *p01.add(1) as u32 + *p11.add(1) as u32) >> 2) as u8,
                    ((*p00.add(2) as u32 + *p10.add(2) as u32 + *p01.add(2) as u32 + *p11.add(2) as u32) >> 2) as u8,
                )
            };
            let idx = (y * hw + x) * 4;
            out[idx] = r;
            out[idx + 1] = g;
            out[idx + 2] = b;
            out[idx + 3] = 255;
        }
    }

    TLS_MAG_FRAME.with(|c| {
        *c.borrow_mut() = Some(MagFallbackFrame { w: hw, h: hh, rgba: out });
    });
    0
}

#[cfg(target_os = "windows")]
struct ReusableDib {
    mem_dc: isize,
    hbmp: isize,
    old_bmp: isize,
    bits: *mut u8,
    w: i32,
    h: i32,
}

#[cfg(target_os = "windows")]
impl ReusableDib {
    fn new(screen_dc: isize) -> Self {
        use std::ffi::c_void;
        extern "system" {
            fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
            fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
        }
        unsafe {
            let gdi32 = LoadLibraryA(b"gdi32.dll\0".as_ptr());
            let p_cdc = GetProcAddress(gdi32, b"CreateCompatibleDC\0".as_ptr());
            let mem_dc = if !p_cdc.is_null() {
                let cdc: unsafe extern "system" fn(isize) -> isize = std::mem::transmute(p_cdc);
                cdc(screen_dc)
            } else {
                0
            };
            Self {
                mem_dc,
                hbmp: 0,
                old_bmp: 0,
                bits: std::ptr::null_mut(),
                w: 0,
                h: 0,
            }
        }
    }

    unsafe fn ensure_size(&mut self, w: i32, h: i32) -> bool {
        use std::ffi::c_void;
        if self.mem_dc == 0 || w <= 0 || h <= 0 {
            return false;
        }
        if self.w >= w && self.h >= h && !self.bits.is_null() {
            return true;
        }
        let alloc_w = w.max(self.w).min(4096);
        let alloc_h = h.max(self.h).min(2560);

        extern "system" {
            fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
            fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
        }
        let gdi32 = LoadLibraryA(b"gdi32.dll\0".as_ptr());
        let p_dib = GetProcAddress(gdi32, b"CreateDIBSection\0".as_ptr());
        let p_sel = GetProcAddress(gdi32, b"SelectObject\0".as_ptr());
        let p_del = GetProcAddress(gdi32, b"DeleteObject\0".as_ptr());
        if p_dib.is_null() || p_sel.is_null() || p_del.is_null() {
            return false;
        }
        let create_dib: unsafe extern "system" fn(isize, *const BitmapInfo, u32, *mut *mut c_void, isize, u32) -> isize =
            std::mem::transmute(p_dib);
        let select_obj: unsafe extern "system" fn(isize, isize) -> isize = std::mem::transmute(p_sel);
        let delete_obj: unsafe extern "system" fn(isize) -> i32 = std::mem::transmute(p_del);

        if self.hbmp != 0 {
            if self.old_bmp != 0 {
                let _ = select_obj(self.mem_dc, self.old_bmp);
            }
            let _ = delete_obj(self.hbmp);
            self.hbmp = 0;
            self.old_bmp = 0;
            self.bits = std::ptr::null_mut();
        }

        let bmi = BitmapInfo {
            bmi_header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: alloc_w,
                bi_height: -alloc_h, // top-down
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: 0, // BI_RGB
                bi_size_image: 0,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            bmi_colors: [0],
        };
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let hbmp = create_dib(self.mem_dc, &bmi, 0, &mut ptr, 0, 0);
        if hbmp == 0 || ptr.is_null() {
            self.w = 0;
            self.h = 0;
            return false;
        }
        self.old_bmp = select_obj(self.mem_dc, hbmp);
        self.hbmp = hbmp;
        self.bits = ptr as *mut u8;
        self.w = alloc_w;
        self.h = alloc_h;
        true
    }

    unsafe fn destroy(&mut self) {
        use std::ffi::c_void;
        extern "system" {
            fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
            fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
        }
        let gdi32 = LoadLibraryA(b"gdi32.dll\0".as_ptr());
        if !gdi32.is_null() {
            let p_sel = GetProcAddress(gdi32, b"SelectObject\0".as_ptr());
            let p_del = GetProcAddress(gdi32, b"DeleteObject\0".as_ptr());
            let p_ddc = GetProcAddress(gdi32, b"DeleteDC\0".as_ptr());
            if !p_sel.is_null() && !p_del.is_null() && self.hbmp != 0 {
                let select_obj: unsafe extern "system" fn(isize, isize) -> isize = std::mem::transmute(p_sel);
                let delete_obj: unsafe extern "system" fn(isize) -> i32 = std::mem::transmute(p_del);
                if self.old_bmp != 0 {
                    let _ = select_obj(self.mem_dc, self.old_bmp);
                }
                let _ = delete_obj(self.hbmp);
            }
            if !p_ddc.is_null() && self.mem_dc != 0 {
                let delete_dc: unsafe extern "system" fn(isize) -> i32 = std::mem::transmute(p_ddc);
                let _ = delete_dc(self.mem_dc);
            }
        }
        self.hbmp = 0;
        self.mem_dc = 0;
        self.bits = std::ptr::null_mut();
    }
}

#[cfg(target_os = "windows")]
struct WallpaperCache {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
    last_check: Instant,
    last_path: Option<PathBuf>,
    last_modified: Option<SystemTime>,
}

#[cfg(target_os = "windows")]
impl WallpaperCache {
    fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            rgba: Vec::new(),
            last_check: Instant::now() - Duration::from_secs(60),
            last_path: None,
            last_modified: None,
        }
    }

    fn refresh(&mut self) {
        if self.last_check.elapsed() < Duration::from_secs(5) && !self.rgba.is_empty() {
            return;
        }
        self.last_check = Instant::now();
        let Some(path) = get_windows_wallpaper_path() else {
            return;
        };
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if self.last_path.as_ref() == Some(&path) && self.last_modified == modified && !self.rgba.is_empty() {
            return;
        }
        let Ok(reader) = image::ImageReader::open(&path) else { return; };
        let Ok(reader) = reader.with_guessed_format() else { return; };
        let Ok(img) = reader.decode() else { return; };
        let resized = img.resize_to_fill(960, 600, image::imageops::FilterType::Triangle);
        let rgba = resized.to_rgba8();
        self.w = 960;
        self.h = 600;
        self.rgba = rgba.into_raw();
        self.last_path = Some(path);
        self.last_modified = modified;
    }

    fn fill_canvas(&self, dst: &mut [u8], out_w: usize, out_h: usize, scene: PhysRect, mon_rect: PhysRect) {
        if self.rgba.is_empty() || self.w == 0 || self.h == 0 || mon_rect.width() <= 0 || mon_rect.height() <= 0 {
            for px in dst.chunks_exact_mut(4) {
                px[0] = 18;
                px[1] = 22;
                px[2] = 32;
                px[3] = 255;
            }
            return;
        }
        let mw = mon_rect.width() as f32;
        let mh = mon_rect.height() as f32;
        let sw = scene.width() as f32;
        let sh = scene.height() as f32;

        for oy in 0..out_h {
            let py = scene.top as f32 + (oy as f32 + 0.5) * (sh / out_h as f32);
            let v = ((py - mon_rect.top as f32) / mh).rem_euclid(1.0);
            let wy = ((v * self.h as f32) as usize).min(self.h - 1);
            let wrow = wy * self.w * 4;
            let drow = oy * out_w * 4;
            for ox in 0..out_w {
                let px = scene.left as f32 + (ox as f32 + 0.5) * (sw / out_w as f32);
                let u = ((px - mon_rect.left as f32) / mw).rem_euclid(1.0);
                let wx = ((u * self.w as f32) as usize).min(self.w - 1);
                let si = wrow + wx * 4;
                let di = drow + ox * 4;
                dst[di] = self.rgba[si];
                dst[di + 1] = self.rgba[si + 1];
                dst[di + 2] = self.rgba[si + 2];
                dst[di + 3] = 255;
            }
        }
    }
}

#[cfg(target_os = "windows")]
struct LayerCapture {
    ext_rect: PhysRect,
    full_rect: PhysRect,
    is_layered: bool,
    lwa_alpha: u8,
    bgra: Vec<u8>,
    dib_stride: usize,
    w_full: i32,
    h_full: i32,
}

#[cfg(target_os = "windows")]
struct OptionalMagFallback {
    initialized: bool,
    host: isize,
    mag: isize,
    last_filtered: isize,
    last_rect: Option<PhysRect>,
    mag_init: unsafe extern "system" fn() -> i32,
    mag_uninit: unsafe extern "system" fn() -> i32,
    mag_set_source: unsafe extern "system" fn(isize, WinRect) -> i32,
    mag_set_filter: unsafe extern "system" fn(isize, u32, i32, *mut isize) -> i32,
    mag_set_cb: unsafe extern "system" fn(isize, unsafe extern "system" fn(isize, *const u8, MagImageHeader, *mut u8, MagImageHeader, WinRect, WinRect, isize) -> i32) -> i32,
    create_win: unsafe extern "system" fn(u32, *const u8, *const u8, u32, i32, i32, i32, i32, isize, isize, isize, *mut std::ffi::c_void) -> isize,
    destroy_win: unsafe extern "system" fn(isize) -> i32,
    set_pos: unsafe extern "system" fn(isize, isize, i32, i32, i32, i32, u32) -> i32,
    show_win: unsafe extern "system" fn(isize, i32) -> i32,
    set_lwa: unsafe extern "system" fn(isize, u32, u8, u32) -> i32,
}

#[cfg(target_os = "windows")]
impl OptionalMagFallback {
    unsafe fn load() -> Option<Self> {
        use std::ffi::c_void;
        extern "system" {
            fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
            fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
        }
        let user32 = LoadLibraryA(b"user32.dll\0".as_ptr());
        let magdll = LoadLibraryA(b"Magnification.dll\0".as_ptr());
        if user32.is_null() || magdll.is_null() {
            return None;
        }
        let p_mi = GetProcAddress(magdll, b"MagInitialize\0".as_ptr());
        let p_mu = GetProcAddress(magdll, b"MagUninitialize\0".as_ptr());
        let p_ms = GetProcAddress(magdll, b"MagSetWindowSource\0".as_ptr());
        let p_mf = GetProcAddress(magdll, b"MagSetWindowFilterList\0".as_ptr());
        let p_mc = GetProcAddress(magdll, b"MagSetImageScalingCallback\0".as_ptr());
        let p_cw = GetProcAddress(user32, b"CreateWindowExA\0".as_ptr());
        let p_dw = GetProcAddress(user32, b"DestroyWindow\0".as_ptr());
        let p_sp = GetProcAddress(user32, b"SetWindowPos\0".as_ptr());
        let p_sw = GetProcAddress(user32, b"ShowWindow\0".as_ptr());
        let p_la = GetProcAddress(user32, b"SetLayeredWindowAttributes\0".as_ptr());
        if p_mi.is_null() || p_mu.is_null() || p_ms.is_null() || p_mf.is_null() || p_mc.is_null()
            || p_cw.is_null() || p_dw.is_null() || p_sp.is_null() || p_sw.is_null() || p_la.is_null()
        {
            return None;
        }
        Some(Self {
            initialized: false,
            host: 0,
            mag: 0,
            last_filtered: 0,
            last_rect: None,
            mag_init: std::mem::transmute(p_mi),
            mag_uninit: std::mem::transmute(p_mu),
            mag_set_source: std::mem::transmute(p_ms),
            mag_set_filter: std::mem::transmute(p_mf),
            mag_set_cb: std::mem::transmute(p_mc),
            create_win: std::mem::transmute(p_cw),
            destroy_win: std::mem::transmute(p_dw),
            set_pos: std::mem::transmute(p_sp),
            show_win: std::mem::transmute(p_sw),
            set_lwa: std::mem::transmute(p_la),
        })
    }

    unsafe fn capture_single_monitor(&mut self, our_hwnd: isize, mon: PhysRect) -> Option<MagFallbackFrame> {
        const WS_EX_NOACTIVATE: u32 = 0x08000000;
        const WS_EX_LAYERED: u32 = 0x00080000;
        const WS_EX_TRANSPARENT: u32 = 0x00000020;
        const WS_EX_TOOLWINDOW: u32 = 0x00000080;
        const WS_POPUP: u32 = 0x80000000;
        const WS_CHILD: u32 = 0x40000000;
        const WS_VISIBLE: u32 = 0x10000000;

        let mw = mon.width();
        let mh = mon.height();
        if mw < 64 || mh < 64 {
            return None;
        }

        if !self.initialized {
            if (self.mag_init)() == 0 {
                return None;
            }
            let host = (self.create_win)(
                WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
                b"STATIC\0".as_ptr(),
                b"sb_fb_h\0".as_ptr(),
                WS_POPUP,
                mon.left,
                mon.top,
                mw,
                mh,
                0,
                0,
                0,
                std::ptr::null_mut(),
            );
            if host == 0 {
                let _ = (self.mag_uninit)();
                return None;
            }
            let _ = (self.set_lwa)(host, 0, 0, 2);
            let mag = (self.create_win)(
                0,
                b"Magnifier\0".as_ptr(),
                b"sb_fb_m\0".as_ptr(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                mw,
                mh,
                host,
                0,
                0,
                std::ptr::null_mut(),
            );
            if mag == 0 {
                let _ = (self.destroy_win)(host);
                let _ = (self.mag_uninit)();
                return None;
            }
            let _ = (self.show_win)(mag, 5);
            let _ = (self.show_win)(host, 8);
            let _ = (self.mag_set_cb)(mag, mag_scaling_callback);
            self.host = host;
            self.mag = mag;
            self.last_rect = Some(mon);
            self.initialized = true;
        }

        if self.last_filtered != our_hwnd {
            let mut list = [our_hwnd, self.host];
            if (self.mag_set_filter)(self.mag, 0, 2, list.as_mut_ptr()) != 0 {
                self.last_filtered = our_hwnd;
            }
        }

        if self.last_rect != Some(mon) {
            let _ = (self.set_pos)(self.host, our_hwnd, mon.left, mon.top, mw, mh, 0x0010 | 0x0200);
            let _ = (self.set_pos)(self.mag, 0, 0, 0, mw, mh, 0x0010 | 0x0004);
            self.last_rect = Some(mon);
        }

        TLS_MAG_FRAME.with(|c| {
            *c.borrow_mut() = None;
        });
        let ok = (self.mag_set_source)(
            self.mag,
            WinRect { left: mon.left, top: mon.top, right: mon.right, bottom: mon.bottom },
        );
        if ok != 0 {
            TLS_MAG_FRAME.with(|c| c.borrow_mut().take())
        } else {
            None
        }
    }

    unsafe fn destroy(&mut self) {
        if self.initialized {
            if self.mag != 0 {
                let _ = (self.destroy_win)(self.mag);
            }
            if self.host != 0 {
                let _ = (self.destroy_win)(self.host);
            }
            let _ = (self.mag_uninit)();
            self.initialized = false;
            self.mag = 0;
            self.host = 0;
        }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_monitors_cb(
    hmon: isize,
    _hdc: isize,
    _lprc: *mut WinRect,
    lparam: isize,
) -> i32 {
    use std::ffi::c_void;
    extern "system" {
        fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    }
    let user32 = LoadLibraryA(b"user32.dll\0".as_ptr());
    let p_gmi = GetProcAddress(user32, b"GetMonitorInfoW\0".as_ptr());
    if !p_gmi.is_null() && lparam != 0 {
        let get_mon: unsafe extern "system" fn(isize, *mut MonitorInfo) -> i32 = std::mem::transmute(p_gmi);
        let mut mi = MonitorInfo {
            cb_size: std::mem::size_of::<MonitorInfo>() as u32,
            rc_monitor: WinRect { left: 0, top: 0, right: 0, bottom: 0 },
            rc_work: WinRect { left: 0, top: 0, right: 0, bottom: 0 },
            dw_flags: 0,
        };
        if get_mon(hmon, &mut mi) != 0 {
            let vec = &mut *(lparam as *mut Vec<PhysRect>);
            vec.push(PhysRect {
                left: mi.rc_monitor.left,
                top: mi.rc_monitor.top,
                right: mi.rc_monitor.right,
                bottom: mi.rc_monitor.bottom,
            });
        }
    }
    1
}

#[cfg(target_os = "windows")]
fn backdrop_worker(shared: Arc<Mutex<SharedCapture>>, stop: Arc<AtomicBool>) {
    use std::ffi::c_void;
    extern "system" {
        fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    }

    unsafe {
        let user32 = LoadLibraryA(b"user32.dll\0".as_ptr());
        let gdi32 = LoadLibraryA(b"gdi32.dll\0".as_ptr());
        let dwmapi = LoadLibraryA(b"dwmapi.dll\0".as_ptr());
        if user32.is_null() || gdi32.is_null() {
            return;
        }

        let p_get_dc = GetProcAddress(user32, b"GetDC\0".as_ptr());
        let p_rel_dc = GetProcAddress(user32, b"ReleaseDC\0".as_ptr());
        let p_get_win = GetProcAddress(user32, b"GetWindow\0".as_ptr());
        let p_vis = GetProcAddress(user32, b"IsWindowVisible\0".as_ptr());
        let p_iconic = GetProcAddress(user32, b"IsIconic\0".as_ptr());
        let p_win_rect = GetProcAddress(user32, b"GetWindowRect\0".as_ptr());
        let p_win_long = GetProcAddress(user32, b"GetWindowLongW\0".as_ptr());
        let p_get_lwa = GetProcAddress(user32, b"GetLayeredWindowAttributes\0".as_ptr());
        let p_cls = GetProcAddress(user32, b"GetClassNameW\0".as_ptr());
        let p_find_ex = GetProcAddress(user32, b"FindWindowExW\0".as_ptr());
        let p_pw = GetProcAddress(user32, b"PrintWindow\0".as_ptr());
        let p_mon_from = GetProcAddress(user32, b"MonitorFromWindow\0".as_ptr());
        let p_mon_info = GetProcAddress(user32, b"GetMonitorInfoW\0".as_ptr());
        let p_enum_mons = GetProcAddress(user32, b"EnumDisplayMonitors\0".as_ptr());
        let p_peek_msg = GetProcAddress(user32, b"PeekMessageA\0".as_ptr());
        let p_trans_msg = GetProcAddress(user32, b"TranslateMessage\0".as_ptr());
        let p_disp_msg = GetProcAddress(user32, b"DispatchMessageA\0".as_ptr());
        let p_gdi_flush = GetProcAddress(gdi32, b"GdiFlush\0".as_ptr());
        let p_dwm_attr = if !dwmapi.is_null() {
            GetProcAddress(dwmapi, b"DwmGetWindowAttribute\0".as_ptr())
        } else {
            std::ptr::null_mut()
        };

        if p_get_dc.is_null() || p_rel_dc.is_null() || p_get_win.is_null() || p_vis.is_null()
            || p_iconic.is_null() || p_win_rect.is_null() || p_win_long.is_null() || p_cls.is_null()
            || p_pw.is_null() || p_mon_from.is_null() || p_mon_info.is_null()
        {
            return;
        }

        let get_dc: unsafe extern "system" fn(isize) -> isize = std::mem::transmute(p_get_dc);
        let rel_dc: unsafe extern "system" fn(isize, isize) -> i32 = std::mem::transmute(p_rel_dc);
        let get_window: unsafe extern "system" fn(isize, u32) -> isize = std::mem::transmute(p_get_win);
        let is_visible: unsafe extern "system" fn(isize) -> i32 = std::mem::transmute(p_vis);
        let is_iconic: unsafe extern "system" fn(isize) -> i32 = std::mem::transmute(p_iconic);
        let get_win_rect: unsafe extern "system" fn(isize, *mut WinRect) -> i32 = std::mem::transmute(p_win_rect);
        let get_win_long: unsafe extern "system" fn(isize, i32) -> i32 = std::mem::transmute(p_win_long);
        let get_cls_name: unsafe extern "system" fn(isize, *mut u16, i32) -> i32 = std::mem::transmute(p_cls);
        let print_window: unsafe extern "system" fn(isize, isize, u32) -> i32 = std::mem::transmute(p_pw);
        let mon_from_win: unsafe extern "system" fn(isize, u32) -> isize = std::mem::transmute(p_mon_from);
        let get_mon_info: unsafe extern "system" fn(isize, *mut MonitorInfo) -> i32 = std::mem::transmute(p_mon_info);

        let screen_dc = get_dc(0);
        let mut dib = ReusableDib::new(screen_dc);
        rel_dc(0, screen_dc);

        let mut mag_fb = OptionalMagFallback::load();
        let mut wp = WallpaperCache::new();

        // Preallocated working buffers (zero per-frame allocations)
        let mut canvas: Vec<u8> = Vec::new();
        let mut blur_a: Vec<u8> = Vec::new();
        let mut blur_b: Vec<u8> = Vec::new();
        let mut blur_hi: Vec<u8> = Vec::new();
        let mut blur_out: Vec<u8> = Vec::new();

        let mut last_scene_rect: Option<PhysRect> = None;
        let mut last_raw_hash: u64 = 0;
        let mut last_blur_key: i32 = -1;
        let mut last_capture_time = Instant::now() - Duration::from_secs(10);

        while !stop.load(Ordering::Relaxed) {
            if !p_peek_msg.is_null() && !p_trans_msg.is_null() && !p_disp_msg.is_null() {
                let peek_msg: unsafe extern "system" fn(*mut Msg, isize, u32, u32, u32) -> i32 =
                    std::mem::transmute(p_peek_msg);
                let trans_msg: unsafe extern "system" fn(*const Msg) -> i32 = std::mem::transmute(p_trans_msg);
                let disp_msg: unsafe extern "system" fn(*const Msg) -> isize = std::mem::transmute(p_disp_msg);
                let mut msg = std::mem::zeroed::<Msg>();
                while peek_msg(&mut msg, 0, 0, 0, 1) != 0 {
                    let _ = trans_msg(&msg);
                    let _ = disp_msg(&msg);
                }
            }

            let (our_hwnd, client_rect_opt, active, blur_level, blur_key) = {
                if let Ok(g) = shared.lock() {
                    (g.our_hwnd, g.client_rect, g.active, g.blur_level, g.blur_key)
                } else {
                    (0, None, false, 6.0, 120)
                }
            };

            if !active || our_hwnd == 0 || client_rect_opt.is_none() {
                std::thread::sleep(Duration::from_millis(60));
                continue;
            }
            let client_rect = client_rect_opt.unwrap();

            // Determine active monitor and whether window is straddling two monitors
            let hmon = mon_from_win(our_hwnd, 2);
            let mut mi = MonitorInfo {
                cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                rc_monitor: WinRect { left: 0, top: 0, right: 1920, bottom: 1080 },
                rc_work: WinRect { left: 0, top: 0, right: 1920, bottom: 1080 },
                dw_flags: 0,
            };
            let _ = get_mon_info(hmon, &mut mi);
            let active_mon = PhysRect {
                left: mi.rc_monitor.left,
                top: mi.rc_monitor.top,
                right: mi.rc_monitor.right,
                bottom: mi.rc_monitor.bottom,
            };

            let mut scene_rect = active_mon;
            let mut straddling_monitors = false;
            if !p_enum_mons.is_null() {
                let enum_mons: unsafe extern "system" fn(isize, *const WinRect, unsafe extern "system" fn(isize, isize, *mut WinRect, isize) -> i32, isize) -> i32 =
                    std::mem::transmute(p_enum_mons);
                let mut mons: Vec<PhysRect> = Vec::with_capacity(4);
                let _ = enum_mons(0, std::ptr::null(), enum_monitors_cb, &mut mons as *mut Vec<PhysRect> as isize);
                let mut touched = 0usize;
                for m in mons {
                    if m.intersects(client_rect).is_some() {
                        scene_rect = scene_rect.union(m);
                        touched += 1;
                    }
                }
                if touched > 1 {
                    straddling_monitors = true;
                }
            }

            let sw = scene_rect.width();
            let sh = scene_rect.height();
            if sw < 32 || sh < 32 {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }

            let slider_changed = blur_key != last_blur_key;
            let scene_changed = last_scene_rect != Some(scene_rect);
            let time_to_refresh = last_capture_time.elapsed() >= Duration::from_millis(120);

            // While dragging across the SAME monitor, scene_rect doesn't change & canvas doesn't need recapturing!
            if !slider_changed && !scene_changed && !time_to_refresh {
                std::thread::sleep(Duration::from_millis(15));
                continue;
            }

            let mut raw_w: usize = 0;
            let mut raw_h: usize = 0;
            let mut needs_reblur = slider_changed || scene_changed || canvas.is_empty();

            if time_to_refresh || scene_changed || canvas.is_empty() {
                last_capture_time = Instant::now();
                wp.refresh();

                // Target downscaled monitor canvas (~768px wide on 1920px monitor -> fast ~1.5ms blur)
                let max_w = if straddling_monitors { 960.0 } else { 768.0 };
                let scale = (max_w / sw as f32).min(540.0 / sh as f32).min(0.5);
                raw_w = ((sw as f32 * scale).round() as usize).clamp(64, 960);
                raw_h = ((sh as f32 * scale).round() as usize).clamp(64, 600);
                let needed = raw_w * raw_h * 4;
                if canvas.len() != needed {
                    canvas.resize(needed, 255);
                }

                // Walk visible windows below our_hwnd top-to-bottom
                let mut layers: Vec<LayerCapture> = Vec::with_capacity(12);
                let mut unredirected_opaque = false;
                let mut h = get_window(our_hwnd, 2); // GW_HWNDNEXT
                let mut iter_guard = 0usize;

                while h != 0 && iter_guard < 160 && layers.len() < 12 {
                    iter_guard += 1;
                    if is_visible(h) != 0 && is_iconic(h) == 0 {
                        let mut cloaked: i32 = 0;
                        if !p_dwm_attr.is_null() {
                            let dwm_attr: unsafe extern "system" fn(isize, u32, *mut c_void, u32) -> i32 =
                                std::mem::transmute(p_dwm_attr);
                            let _ = dwm_attr(h, 14, &mut cloaked as *mut i32 as *mut c_void, 4);
                        }
                        if cloaked == 0 {
                            let mut cls_buf = [0u16; 64];
                            let clen = get_cls_name(h, cls_buf.as_mut_ptr(), 64).max(0) as usize;
                            let cls_name = String::from_utf16_lossy(&cls_buf[..clen]);

                            let skip_shell = matches!(
                                cls_name.as_str(),
                                "EdgeUiInputWndClass"
                                    | "EdgeUiInputTopWndClass"
                                    | "DummyDwmListenerWindow"
                                    | "ForegroundStaging"
                                    | "ApplicationManager_DesktopShellWindow"
                                    | "LockScreenBackstopFrame"
                                    | "Shell_Dim"
                                    | "IME"
                                    | "MSCTFIME UI"
                                    | "tooltips_class32"
                            ) || (cls_name == "WorkerW" && {
                                if !p_find_ex.is_null() {
                                    let find_ex: unsafe extern "system" fn(isize, isize, *const u16, *const u16) -> isize =
                                        std::mem::transmute(p_find_ex);
                                    let defview_name: Vec<u16> = "SHELLDLL_DefView\0".encode_utf16().collect();
                                    find_ex(h, 0, defview_name.as_ptr(), std::ptr::null()) == 0
                                } else {
                                    true
                                }
                            });

                            if !skip_shell {
                                let ex_style = get_win_long(h, -20);
                                let is_layered = (ex_style & 0x00080000) != 0;
                                let mut lwa_alpha = 255u8;
                                let mut skip_alpha0 = false;
                                if is_layered && !p_get_lwa.is_null() {
                                    let get_lwa: unsafe extern "system" fn(isize, *mut u32, *mut u8, *mut u32) -> i32 =
                                        std::mem::transmute(p_get_lwa);
                                    let mut key = 0u32;
                                    let mut flags = 0u32;
                                    if get_lwa(h, &mut key, &mut lwa_alpha, &mut flags) != 0
                                        && (flags & 2) != 0
                                        && lwa_alpha == 0
                                    {
                                        skip_alpha0 = true;
                                    }
                                }

                                if !skip_alpha0 {
                                    let mut full_wr = WinRect { left: 0, top: 0, right: 0, bottom: 0 };
                                    if get_win_rect(h, &mut full_wr) != 0 {
                                        let full_rect = PhysRect {
                                            left: full_wr.left,
                                            top: full_wr.top,
                                            right: full_wr.right,
                                            bottom: full_wr.bottom,
                                        };
                                        let mut ext_wr = full_wr;
                                        if !p_dwm_attr.is_null() {
                                            let dwm_attr: unsafe extern "system" fn(isize, u32, *mut c_void, u32) -> i32 =
                                                std::mem::transmute(p_dwm_attr);
                                            let _ = dwm_attr(h, 9, &mut ext_wr as *mut WinRect as *mut c_void, 16);
                                        }
                                        let ext_rect = PhysRect {
                                            left: ext_wr.left,
                                            top: ext_wr.top,
                                            right: ext_wr.right,
                                            bottom: ext_wr.bottom,
                                        };

                                        if ext_rect.intersects(scene_rect).is_some()
                                            && full_rect.width() > 8
                                            && full_rect.height() > 8
                                            && full_rect.width() <= 4096
                                            && full_rect.height() <= 2560
                                        {
                                            let wf = full_rect.width();
                                            let hf = full_rect.height();
                                            if dib.ensure_size(wf, hf) {
                                                // Zero top slice of DIB to detect blank/failed captures cleanly
                                                let dib_stride = (dib.w as usize) * 4;
                                                for row in 0..(hf as usize) {
                                                    std::ptr::write_bytes(dib.bits.add(row * dib_stride), 0, (wf as usize) * 4);
                                                }
                                                let ok = print_window(h, dib.mem_dc, 2); // PW_RENDERFULLCONTENT
                                                if !p_gdi_flush.is_null() {
                                                    let flush: unsafe extern "system" fn() -> i32 = std::mem::transmute(p_gdi_flush);
                                                    let _ = flush();
                                                }

                                                let mut has_pixels = false;
                                                if ok != 0 {
                                                    let sy_step = (hf as usize / 14).max(1);
                                                    let sx_step = (wf as usize / 14).max(1);
                                                    'chk: for sy in (0..(hf as usize)).step_by(sy_step) {
                                                        let row = dib.bits.add(sy * dib_stride);
                                                        for sx in (0..(wf as usize)).step_by(sx_step) {
                                                            let p = row.add(sx * 4);
                                                            if *p != 0 || *p.add(1) != 0 || *p.add(2) != 0 || *p.add(3) != 0 {
                                                                has_pixels = true;
                                                                break 'chk;
                                                            }
                                                        }
                                                    }
                                                }

                                                if has_pixels {
                                                    let mut bgra = vec![0u8; (hf as usize) * (wf as usize) * 4];
                                                    for row in 0..(hf as usize) {
                                                        std::ptr::copy_nonoverlapping(
                                                            dib.bits.add(row * dib_stride),
                                                            bgra.as_mut_ptr().add(row * (wf as usize) * 4),
                                                            (wf as usize) * 4,
                                                        );
                                                    }
                                                    let covers_all = !is_layered && ext_rect.contains(scene_rect);
                                                    layers.push(LayerCapture {
                                                        ext_rect,
                                                        full_rect,
                                                        is_layered,
                                                        lwa_alpha,
                                                        bgra,
                                                        dib_stride: (wf as usize) * 4,
                                                        w_full: wf,
                                                        h_full: hf,
                                                    });
                                                    if covers_all {
                                                        break;
                                                    }
                                                } else if !is_layered && ext_rect.intersects(client_rect).is_some() {
                                                    // Large opaque unredirected window (e.g. DX11/Vulkan game) failed PrintWindow
                                                    unredirected_opaque = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    h = get_window(h, 2);
                }

                let mut used_mag = false;
                if unredirected_opaque && !straddling_monitors {
                    if let Some(ref mut fb) = mag_fb {
                        if let Some(mag_frame) = fb.capture_single_monitor(our_hwnd, active_mon) {
                            raw_w = mag_frame.w;
                            raw_h = mag_frame.h;
                            canvas = mag_frame.rgba;
                            scene_rect = active_mon;
                            used_mag = true;
                        }
                    }
                }

                if !used_mag {
                    // Base: aligned desktop wallpaper
                    wp.fill_canvas(&mut canvas, raw_w, raw_h, scene_rect, active_mon);

                    // Composite windows bottom-to-top onto monitor canvas
                    for layer in layers.iter().rev() {
                        let Some(inter) = layer.ext_rect.intersects(scene_rect) else {
                            continue;
                        };
                        let ox0 = (((inter.left - scene_rect.left) as f32 / sw as f32) * raw_w as f32)
                            .floor()
                            .clamp(0.0, raw_w as f32) as usize;
                        let oy0 = (((inter.top - scene_rect.top) as f32 / sh as f32) * raw_h as f32)
                            .floor()
                            .clamp(0.0, raw_h as f32) as usize;
                        let ox1 = (((inter.right - scene_rect.left) as f32 / sw as f32) * raw_w as f32)
                            .ceil()
                            .clamp(0.0, raw_w as f32) as usize;
                        let oy1 = (((inter.bottom - scene_rect.top) as f32 / sh as f32) * raw_h as f32)
                            .ceil()
                            .clamp(0.0, raw_h as f32) as usize;

                        for oy in oy0..oy1 {
                            let py = scene_rect.top + ((oy as f32 + 0.5) * sh as f32 / raw_h as f32) as i32;
                            let sy = (py - layer.full_rect.top).clamp(0, layer.h_full - 1) as usize;
                            let src_row = sy * layer.dib_stride;
                            let dst_row = oy * raw_w * 4;

                            for ox in ox0..ox1 {
                                let px = scene_rect.left + ((ox as f32 + 0.5) * sw as f32 / raw_w as f32) as i32;
                                let sx = (px - layer.full_rect.left).clamp(0, layer.w_full - 1) as usize;
                                let si = src_row + sx * 4;
                                let b = layer.bgra[si];
                                let g = layer.bgra[si + 1];
                                let r = layer.bgra[si + 2];
                                let a = layer.bgra[si + 3];
                                let di = dst_row + ox * 4;

                                if !layer.is_layered {
                                    canvas[di] = r;
                                    canvas[di + 1] = g;
                                    canvas[di + 2] = b;
                                    canvas[di + 3] = 255;
                                } else {
                                    let eff_a = if a > 0 {
                                        ((a as u16 * layer.lwa_alpha as u16) / 255) as u8
                                    } else if r != 0 || g != 0 || b != 0 {
                                        layer.lwa_alpha
                                    } else {
                                        0
                                    };
                                    if eff_a == 255 {
                                        canvas[di] = r;
                                        canvas[di + 1] = g;
                                        canvas[di + 2] = b;
                                        canvas[di + 3] = 255;
                                    } else if eff_a > 0 {
                                        let af = eff_a as u16;
                                        let inv = 255 - af;
                                        canvas[di] = ((r as u16 * af + canvas[di] as u16 * inv) / 255) as u8;
                                        canvas[di + 1] = ((g as u16 * af + canvas[di + 1] as u16 * inv) / 255) as u8;
                                        canvas[di + 2] = ((b as u16 * af + canvas[di + 2] as u16 * inv) / 255) as u8;
                                        canvas[di + 3] = 255;
                                    }
                                }
                            }
                        }
                    }
                }

                // Strided hash of raw canvas
                let mut hval: u64 = 14695981039346656037;
                for i in (0..canvas.len()).step_by(96) {
                    hval = hval.wrapping_mul(1099511628211) ^ (canvas[i] as u64);
                }
                if hval != last_raw_hash {
                    last_raw_hash = hval;
                    needs_reblur = true;
                }
            } else {
                // Only slider_changed
                if let Some(prev) = last_scene_rect {
                    scene_rect = prev;
                }
                if let Ok(g) = shared.lock() {
                    if let Some(ref lf) = g.latest {
                        raw_w = lf.w;
                        raw_h = lf.h;
                    } else {
                        needs_reblur = false;
                    }
                }
            }

            if needs_reblur && raw_w >= 8 && raw_h >= 8 && canvas.len() == raw_w * raw_h * 4 {
                let r_f = 0.25 + blur_level * 0.9 + (blur_level * blur_level) * 0.22;
                let r_low = r_f.floor() as usize;
                let frac = (r_f - r_low as f32).clamp(0.0, 1.0);

                box_blur_3pass_into(&canvas, &mut blur_a, &mut blur_b, raw_w, raw_h, r_low);
                if frac > 0.04 {
                    box_blur_3pass_into(&canvas, &mut blur_hi, &mut blur_b, raw_w, raw_h, r_low + 1);
                    lerp_rgba_into(&blur_a, &blur_hi, &mut blur_out, frac);
                } else {
                    blur_out.resize(blur_a.len(), 255);
                    blur_out.copy_from_slice(&blur_a);
                }

                last_scene_rect = Some(scene_rect);
                last_blur_key = blur_key;

                if let Ok(mut g) = shared.lock() {
                    g.seq_counter = g.seq_counter.wrapping_add(1).max(1);
                    let seq = g.seq_counter;
                    g.latest = Some(BlurredFrame {
                        w: raw_w,
                        h: raw_h,
                        rgba: blur_out.clone(),
                        scene_rect,
                        seq,
                    });
                    if let Some(ref ctx) = g.ctx {
                        ctx.request_repaint();
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(16));
        }

        if let Some(ref mut fb) = mag_fb {
            fb.destroy();
        }
        dib.destroy();
    }
}

#[cfg(target_os = "windows")]
fn get_windows_wallpaper_path() -> Option<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let tc = PathBuf::from(appdata).join("Microsoft\\Windows\\Themes\\TranscodedWallpaper");
        if tc.exists() {
            return Some(tc);
        }
    }
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    if let Ok(desktop) = hkcu.open_subkey("Control Panel\\Desktop") {
        if let Ok(wp) = desktop.get_value::<String, _>("WallPaper") {
            let p = PathBuf::from(wp.trim());
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

fn lerp_rgba_into(a: &[u8], b: &[u8], out: &mut Vec<u8>, t: f32) {
    let inv = 1.0 - t;
    out.resize(a.len(), 255);
    for i in 0..a.len() {
        out[i] = ((a[i] as f32 * inv + b[i] as f32 * t).round()).clamp(0.0, 255.0) as u8;
    }
}

/// Fast O(W*H) 3-pass separable box blur using caller-owned reusable scratch buffers
fn box_blur_3pass_into(
    src: &[u8],
    buf_a: &mut Vec<u8>,
    buf_b: &mut Vec<u8>,
    w: usize,
    h: usize,
    radius: usize,
) {
    buf_a.resize(src.len(), 255);
    buf_a.copy_from_slice(src);
    if radius == 0 || w == 0 || h == 0 {
        return;
    }
    buf_b.resize(src.len(), 255);
    for _ in 0..3 {
        box_blur_h(buf_a, buf_b, w, h, radius);
        box_blur_v(buf_b, buf_a, w, h, radius);
    }
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, radius: usize) {
    let win = (radius * 2 + 1) as u32;
    let r = radius as isize;
    let wi = w as isize;

    for y in 0..h {
        let row = y * w * 4;
        let mut sr: u32 = 0;
        let mut sg: u32 = 0;
        let mut sb: u32 = 0;

        for k in -r..=r {
            let sx = k.clamp(0, wi - 1) as usize;
            let idx = row + sx * 4;
            sr += src[idx] as u32;
            sg += src[idx + 1] as u32;
            sb += src[idx + 2] as u32;
        }

        for x in 0..w {
            let out_idx = row + x * 4;
            dst[out_idx] = (sr / win) as u8;
            dst[out_idx + 1] = (sg / win) as u8;
            dst[out_idx + 2] = (sb / win) as u8;
            dst[out_idx + 3] = 255;

            let left_x = (x as isize - r).clamp(0, wi - 1) as usize;
            let right_x = (x as isize + r + 1).clamp(0, wi - 1) as usize;
            let li = row + left_x * 4;
            let ri = row + right_x * 4;
            sr = sr + src[ri] as u32 - src[li] as u32;
            sg = sg + src[ri + 1] as u32 - src[li + 1] as u32;
            sb = sb + src[ri + 2] as u32 - src[li + 2] as u32;
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, radius: usize) {
    let win = (radius * 2 + 1) as u32;
    let r = radius as isize;
    let hi = h as isize;
    let stride = w * 4;

    for x in 0..w {
        let col = x * 4;
        let mut sr: u32 = 0;
        let mut sg: u32 = 0;
        let mut sb: u32 = 0;

        for k in -r..=r {
            let sy = k.clamp(0, hi - 1) as usize;
            let idx = sy * stride + col;
            sr += src[idx] as u32;
            sg += src[idx + 1] as u32;
            sb += src[idx + 2] as u32;
        }

        for y in 0..h {
            let out_idx = y * stride + col;
            dst[out_idx] = (sr / win) as u8;
            dst[out_idx + 1] = (sg / win) as u8;
            dst[out_idx + 2] = (sb / win) as u8;
            dst[out_idx + 3] = 255;

            let top_y = (y as isize - r).clamp(0, hi - 1) as usize;
            let bot_y = (y as isize + r + 1).clamp(0, hi - 1) as usize;
            let ti = top_y * stride + col;
            let bi = bot_y * stride + col;
            sr = sr + src[bi] as u32 - src[ti] as u32;
            sg = sg + src[bi + 1] as u32 - src[ti + 1] as u32;
            sb = sb + src[bi + 2] as u32 - src[ti + 2] as u32;
        }
    }
}
