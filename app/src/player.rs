//! Plays a sound or video file inside Blockwise (the Assets page), with no
//! other app. On the Mac this is AVFoundation: an AVPlayer plays the file and
//! its sound, and each new video picture is copied into an egui texture.
//! Elsewhere `Media::open` gives nothing and the page says so.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::path::Path;

/// One open sound or video.
pub struct Media {
    #[cfg(target_os = "macos")]
    mac: mac::Player,
    tex: Option<TextureHandle>,
    looping: bool,
    /// Set by `tick` when the file reached its end (and is not looping).
    pub ended: bool,
}

impl Media {
    /// `video`: also take the pictures (a sound file has none).
    pub fn open(path: &Path, video: bool) -> Option<Media> {
        Media::open_source(path.to_str()?, video)
    }

    /// A file path or a web address (http or https).
    pub fn open_source(src: &str, video: bool) -> Option<Media> {
        #[cfg(target_os = "macos")]
        {
            let mac = mac::Player::open(src, video)?;
            Some(Media { mac, tex: None, looping: false, ended: false })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (src, video);
            None
        }
    }

    pub fn play(&mut self) {
        #[cfg(target_os = "macos")]
        {
            // from the end: start again
            let d = self.duration();
            if self.ended || (d > 0.0 && self.position() >= d - 0.05) {
                self.mac.seek(0.0);
            }
            self.mac.play();
        }
        self.ended = false;
    }

    pub fn pause(&mut self) {
        #[cfg(target_os = "macos")]
        self.mac.pause();
    }

    pub fn toggle(&mut self) {
        if self.is_playing() {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn is_playing(&self) -> bool {
        #[cfg(target_os = "macos")]
        return self.mac.rate() != 0.0;
        #[cfg(not(target_os = "macos"))]
        false
    }

    /// Seconds from the start.
    pub fn position(&self) -> f64 {
        #[cfg(target_os = "macos")]
        return self.mac.position();
        #[cfg(not(target_os = "macos"))]
        0.0
    }

    /// Seconds; 0 until the file is read.
    pub fn duration(&self) -> f64 {
        #[cfg(target_os = "macos")]
        return self.mac.duration();
        #[cfg(not(target_os = "macos"))]
        0.0
    }

    pub fn seek(&mut self, secs: f64) {
        self.ended = false;
        #[cfg(target_os = "macos")]
        self.mac.seek(secs.max(0.0));
        #[cfg(not(target_os = "macos"))]
        let _ = secs;
    }

    /// 0 to 1.
    pub fn set_volume(&mut self, v: f32) {
        #[cfg(target_os = "macos")]
        self.mac.set_volume(v.clamp(0.0, 1.0));
        #[cfg(not(target_os = "macos"))]
        let _ = v;
    }

    pub fn set_looping(&mut self, on: bool) {
        self.looping = on;
    }

    pub fn looping(&self) -> bool {
        self.looping
    }

    /// Once a frame: takes the newest video picture, notices the end, and
    /// asks for the next frame while playing.
    pub fn tick(&mut self, ctx: &Context) {
        #[cfg(target_os = "macos")]
        {
            if let Some(img) = self.mac.frame() {
                match &mut self.tex {
                    Some(t) => t.set(img, TextureOptions::LINEAR),
                    None => self.tex = Some(ctx.load_texture("asset video", img, TextureOptions::LINEAR)),
                }
            }
            let d = self.duration();
            if !self.is_playing() && d > 0.0 && self.position() >= d - 0.05 && !self.ended {
                if self.looping {
                    self.mac.seek(0.0);
                    self.mac.play();
                } else {
                    self.ended = true;
                }
            }
            // keep drawing while it plays, and until the file is read
            if self.is_playing() || d == 0.0 || self.tex.is_none() {
                ctx.request_repaint_after(std::time::Duration::from_millis(15));
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = ctx;
    }

    /// The video's newest picture (none for a sound, or before the first).
    pub fn picture(&self) -> Option<&TextureHandle> {
        self.tex.as_ref()
    }
}

/// A frame of BGRA pixels, rows `stride` bytes apart, as RGBA.
pub fn bgra_to_image(w: usize, h: usize, stride: usize, bytes: &[u8]) -> ColorImage {
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &bytes[y * stride..y * stride + w * 4];
        for px in row.chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    ColorImage::from_rgba_unmultiplied([w, h], &rgba)
}

/// The offline preview's real players: each video widget and sound player
/// of the running screen plays its file, following what the blocks asked
/// (the runtime), and tells the runtime where it is and when it ended.
#[derive(Default)]
pub struct Preview {
    run_id: u64,
    videos: std::collections::HashMap<String, Slot>,
    sounds: std::collections::HashMap<String, Slot>,
}

struct Slot {
    source: String,
    /// Sounds: the runtime's count of "play" blocks this one is for.
    gen: u64,
    media: Media,
    /// Where it was put last frame (ms): a different value is a "go to".
    written: f64,
    was_playing: bool,
}

/// A project file by name, a file on this computer by its whole path, or a web address.
fn source(rt: &crate::runtime::Runtime, name: &str, kind: crate::model::FileKind) -> Option<String> {
    if name.starts_with("http://") || name.starts_with("https://") {
        return Some(name.to_string());
    }
    if let Some(f) = rt.project.file(name).filter(|f| f.kind == kind) {
        return rt.project.file_path(f).to_str().map(String::from);
    }
    let p = Path::new(name.strip_prefix("file://").unwrap_or(name));
    (p.is_absolute() && p.is_file()).then(|| p.to_str().map(String::from)).flatten()
}

/// Where the phone view finds a video widget's newest picture.
pub fn frame_id(widget: &str) -> egui::Id {
    egui::Id::new(("preview-video", widget))
}

impl Preview {
    /// Everything stops (the preview closed).
    pub fn clear(&mut self) {
        self.videos.clear();
        self.sounds.clear();
    }

    /// Once a frame while the preview runs.
    pub fn sync(&mut self, rt: &mut crate::runtime::Runtime, ctx: &Context) {
        use crate::model::{FileKind, WKind};
        if rt.run_id != self.run_id {
            self.clear();
            self.run_id = rt.run_id;
        }
        // stopped at a breakpoint or crashed: everything waits
        if rt.pause.is_some() || rt.crash.is_some() {
            for s in self.videos.values_mut().chain(self.sounds.values_mut()) {
                s.media.pause();
                s.was_playing = false;
            }
            return;
        }
        let screen = rt.screen_idx;
        let ids: Vec<String> = rt.project.screens[screen].widgets.iter().filter(|w| w.kind == WKind::VideoView).map(|w| w.id.clone()).collect();
        let keys: Vec<String> = ids.iter().map(|id| format!("{screen}:{id}")).collect();
        self.videos.retain(|k, _| keys.contains(k));
        let mut ended = vec![];
        for (id, key) in ids.iter().zip(&keys) {
            let Some(name) = rt.widgets.get(id).map(|st| st.video.clone()) else { continue };
            let src = source(rt, &name, FileKind::Video);
            let Some(src) = src else {
                self.videos.remove(key);
                continue;
            };
            if self.videos.get(key).map_or(true, |s| s.source != src) {
                match Media::open_source(&src, true) {
                    Some(media) => {
                        self.videos.insert(key.clone(), Slot { source: src, gen: 0, media, written: 0.0, was_playing: false });
                    }
                    None => {
                        self.videos.remove(key);
                        continue;
                    }
                }
            }
            let (Some(slot), Some(st)) = (self.videos.get_mut(key), rt.widgets.get_mut(id)) else { continue };
            let m = &mut slot.media;
            m.set_looping(st.looping);
            if (st.video_pos - slot.written).abs() > 1.0 {
                m.seek(st.video_pos / 1000.0);
            }
            if st.playing && !slot.was_playing {
                m.play();
            } else if !st.playing && m.is_playing() {
                m.pause();
            }
            m.tick(ctx);
            if st.playing && m.ended {
                st.playing = false;
                ended.push(id.clone());
            }
            slot.was_playing = st.playing;
            st.video_pos = (m.position() * 1000.0).round();
            slot.written = st.video_pos;
            match m.picture() {
                Some(t) => ctx.data_mut(|d| d.insert_temp(frame_id(id), t.clone())),
                None => ctx.data_mut(|d| d.remove::<egui::TextureHandle>(frame_id(id))),
            }
        }
        for id in ended {
            rt.fire(&id, "onCompletion", std::collections::HashMap::new());
        }
        // sounds: one real player for each sound player that was given a sound
        let names: Vec<String> = rt.sounds.keys().cloned().collect();
        self.sounds.retain(|k, _| names.contains(k));
        for name in names {
            let job = rt.sounds[&name].clone();
            if job.source.is_empty() {
                self.sounds.remove(&name);
                continue;
            }
            if self.sounds.get(&name).map_or(true, |s| s.gen != job.gen) {
                let opened = source(rt, &job.source, FileKind::Sound).and_then(|src| Media::open_source(&src, false).map(|m| (src, m)));
                match opened {
                    Some((src, media)) => {
                        self.sounds.insert(name.clone(), Slot { source: src, gen: job.gen, media, written: 0.0, was_playing: false });
                    }
                    None => {
                        self.sounds.remove(&name);
                        rt.playing.remove(&name);
                        continue;
                    }
                }
            }
            let Some(slot) = self.sounds.get_mut(&name) else { continue };
            let m = &mut slot.media;
            m.set_looping(job.looping);
            m.set_volume(job.volume);
            if let Some(ms) = rt.sounds.get_mut(&name).and_then(|j| j.seek.take()) {
                m.seek(ms / 1000.0);
            }
            let want = rt.playing.contains(&name);
            if want && !slot.was_playing {
                m.play();
            } else if !want && m.is_playing() {
                m.pause();
            }
            m.tick(ctx);
            if want && m.ended {
                rt.playing.remove(&name);
            }
            slot.was_playing = rt.playing.contains(&name);
            if let Some(j) = rt.sounds.get_mut(&name) {
                j.pos = (m.position() * 1000.0).round();
                j.len = (m.duration() * 1000.0).round();
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::encode::{Encode, Encoding, RefEncode};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::{msg_send, msg_send_id};
    use std::ffi::CString;

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct CMTime {
        value: i64,
        timescale: i32,
        flags: u32,
        epoch: i64,
    }
    unsafe impl Encode for CMTime {
        const ENCODING: Encoding = Encoding::Struct("?", &[Encoding::LongLong, Encoding::Int, Encoding::UInt, Encoding::LongLong]);
    }
    unsafe impl RefEncode for CMTime {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }
    const ZERO: CMTime = CMTime { value: 0, timescale: 1, flags: 1, epoch: 0 };

    /// A CVPixelBuffer.
    #[repr(C)]
    pub struct CVBuffer {
        _p: [u8; 0],
    }
    unsafe impl RefEncode for CVBuffer {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CVBuffer", &[]));
    }

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}
    #[link(name = "CoreMedia", kind = "framework")]
    extern "C" {
        fn CMTimeGetSeconds(t: CMTime) -> f64;
        fn CMTimeMakeWithSeconds(seconds: f64, scale: i32) -> CMTime;
    }
    #[link(name = "CoreVideo", kind = "framework")]
    extern "C" {
        static kCVPixelBufferPixelFormatTypeKey: *const AnyObject;
        fn CVPixelBufferLockBaseAddress(b: *mut CVBuffer, flags: u64) -> i32;
        fn CVPixelBufferUnlockBaseAddress(b: *mut CVBuffer, flags: u64) -> i32;
        fn CVPixelBufferGetBaseAddress(b: *mut CVBuffer) -> *const u8;
        fn CVPixelBufferGetBytesPerRow(b: *mut CVBuffer) -> usize;
        fn CVPixelBufferGetWidth(b: *mut CVBuffer) -> usize;
        fn CVPixelBufferGetHeight(b: *mut CVBuffer) -> usize;
        fn CVPixelBufferRelease(b: *mut CVBuffer);
    }
    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {
        fn CACurrentMediaTime() -> f64;
    }

    pub struct Player {
        player: Retained<AnyObject>,
        item: Retained<AnyObject>,
        output: Option<Retained<AnyObject>>,
    }

    fn class(name: &str) -> Option<&'static AnyClass> {
        AnyClass::get(name)
    }

    impl Player {
        pub fn open(src: &str, video: bool) -> Option<Player> {
            let c = CString::new(src).ok()?;
            let web = src.starts_with("http://") || src.starts_with("https://");
            unsafe {
                let s: Option<Retained<AnyObject>> = msg_send_id![class("NSString")?, stringWithUTF8String: c.as_ptr()];
                let url: Option<Retained<AnyObject>> = if web {
                    msg_send_id![class("NSURL")?, URLWithString: &*s?]
                } else {
                    msg_send_id![class("NSURL")?, fileURLWithPath: &*s?]
                };
                let item: Option<Retained<AnyObject>> = msg_send_id![class("AVPlayerItem")?, playerItemWithURL: &*url?];
                let item = item?;
                let output = if video {
                    // pictures as BGRA, which turns into RGBA by swapping two bytes
                    let format: Retained<AnyObject> = msg_send_id![class("NSNumber")?, numberWithUnsignedInt: u32::from_be_bytes(*b"BGRA")];
                    let key = &*kCVPixelBufferPixelFormatTypeKey;
                    let attrs: Retained<AnyObject> = msg_send_id![class("NSDictionary")?, dictionaryWithObject: &*format, forKey: key];
                    let out: Option<Retained<AnyObject>> = msg_send_id![msg_send_id![class("AVPlayerItemVideoOutput")?, alloc], initWithPixelBufferAttributes: &*attrs];
                    let out = out?;
                    let _: () = msg_send![&*item, addOutput: &*out];
                    Some(out)
                } else {
                    None
                };
                let player: Option<Retained<AnyObject>> = msg_send_id![class("AVPlayer")?, playerWithPlayerItem: &*item];
                let player = player?;
                // play now, not after filling a buffer
                let _: () = msg_send![&*player, setAutomaticallyWaitsToMinimizeStalling: Bool::NO];
                Some(Player { player, item, output })
            }
        }

        pub fn play(&self) {
            unsafe {
                let _: () = msg_send![&*self.player, play];
            }
        }

        pub fn pause(&self) {
            unsafe {
                let _: () = msg_send![&*self.player, pause];
            }
        }

        pub fn rate(&self) -> f32 {
            unsafe { msg_send![&*self.player, rate] }
        }

        pub fn position(&self) -> f64 {
            let t: CMTime = unsafe { msg_send![&*self.player, currentTime] };
            let s = unsafe { CMTimeGetSeconds(t) };
            if s.is_finite() {
                s.max(0.0)
            } else {
                0.0
            }
        }

        pub fn duration(&self) -> f64 {
            let t: CMTime = unsafe { msg_send![&*self.item, duration] };
            let s = unsafe { CMTimeGetSeconds(t) };
            if s.is_finite() && s > 0.0 {
                s
            } else {
                0.0
            }
        }

        pub fn seek(&self, secs: f64) {
            unsafe {
                let t = CMTimeMakeWithSeconds(secs, 600);
                let _: () = msg_send![&*self.player, seekToTime: t, toleranceBefore: ZERO, toleranceAfter: ZERO];
            }
        }

        pub fn set_volume(&self, v: f32) {
            unsafe {
                let _: () = msg_send![&*self.player, setVolume: v];
            }
        }

        /// The newest picture, if there is one not taken yet.
        pub fn frame(&self) -> Option<egui::ColorImage> {
            let out = self.output.as_ref()?;
            unsafe {
                let t: CMTime = msg_send![&**out, itemTimeForHostTime: CACurrentMediaTime()];
                let fresh: Bool = msg_send![&**out, hasNewPixelBufferForItemTime: t];
                if !fresh.as_bool() {
                    return None;
                }
                let buf: *mut CVBuffer = msg_send![&**out, copyPixelBufferForItemTime: t, itemTimeForDisplay: std::ptr::null_mut::<CMTime>()];
                if buf.is_null() {
                    return None;
                }
                CVPixelBufferLockBaseAddress(buf, 1);
                let (w, h, stride) = (CVPixelBufferGetWidth(buf), CVPixelBufferGetHeight(buf), CVPixelBufferGetBytesPerRow(buf));
                let base = CVPixelBufferGetBaseAddress(buf);
                let img = (!base.is_null() && w > 0 && h > 0 && stride >= w * 4).then(|| {
                    let bytes = std::slice::from_raw_parts(base, stride * h);
                    super::bgra_to_image(w, h, stride, bytes)
                });
                CVPixelBufferUnlockBaseAddress(buf, 1);
                CVPixelBufferRelease(buf);
                img
            }
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            unsafe {
                let _: () = msg_send![&*self.player, pause];
                let none: *const AnyObject = std::ptr::null();
                let _: () = msg_send![&*self.player, replaceCurrentItemWithPlayerItem: none];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_rows_with_padding_become_rgba() {
        // 2 × 2, rows padded to 12 bytes
        let mut b = vec![0u8; 24];
        b[0..4].copy_from_slice(&[1, 2, 3, 9]);
        b[4..8].copy_from_slice(&[4, 5, 6, 9]);
        b[12..16].copy_from_slice(&[7, 8, 9, 9]);
        b[16..20].copy_from_slice(&[10, 11, 12, 9]);
        let img = bgra_to_image(2, 2, 12, &b);
        assert_eq!(img.size, [2, 2]);
        let px: Vec<[u8; 4]> = img.pixels.iter().map(|c| c.to_array()).collect();
        assert_eq!(px, vec![[3, 2, 1, 255], [6, 5, 4, 255], [9, 8, 7, 255], [12, 11, 10, 255]]);
    }
}
