//! macOS VideoToolbox hardware H.264 encoder.
//!
//! Output matches Swift WebDock: AVCC length-prefixed AUs + avcC for WebCodecs.
//! Critical: only mark `keyframe` when an IDR NAL (type 5) is present — SEI-only
//! samples with ForceKeyFrame used to black-screen the browser decoder.

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::ptr;
use std::sync::{Condvar, Mutex};

use tracing::{debug, warn};

use crate::h264::{build_avcc, H264Encoded, H264Error};

type OSStatus = i32;
type CFIndex = isize;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFMutableDictionaryRef = *mut c_void;
type CFNumberRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CVPixelBufferRef = *mut c_void;
type CVPixelBufferPoolRef = *mut c_void;
type CMSampleBufferRef = *mut c_void;
type CMFormatDescriptionRef = *const c_void;
type CMBlockBufferRef = *mut c_void;
type VTCompressionSessionRef = *mut c_void;
type VTEncodeInfoFlags = u32;

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

// 'BGRA'
const K_CV_PIXEL_FORMAT_TYPE_32_BGRA: u32 = 0x4247_5241;
const K_CV_RETURN_SUCCESS: i32 = 0;
const K_CM_BLOCK_BUFFER_NO_ERR: i32 = 0;
const K_VT_ENCODE_INFO_FRAME_DROPPED: u32 = 1 << 1;
const K_CF_NUMBER_SINT32_TYPE: i32 = 3;
const K_CF_NUMBER_FLOAT64_TYPE: i32 = 6;
const K_CM_VIDEO_CODEC_TYPE_H264: u32 = 0x6176_6331; // 'avc1'

#[repr(C)]
struct CFDictionaryKeyCallBacks {
    _private: [u8; 64],
}
#[repr(C)]
struct CFDictionaryValueCallBacks {
    _private: [u8; 64],
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFDictionaryCreateMutable(
        allocator: CFAllocatorRef,
        capacity: CFIndex,
        key_callbacks: *const CFDictionaryKeyCallBacks,
        value_callbacks: *const CFDictionaryValueCallBacks,
    ) -> CFMutableDictionaryRef;
    fn CFDictionarySetValue(dict: CFMutableDictionaryRef, key: *const c_void, value: *const c_void);
    fn CFNumberCreate(
        allocator: CFAllocatorRef,
        the_type: i32,
        value_ptr: *const c_void,
    ) -> CFNumberRef;
    static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
    static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;
    static kCFBooleanTrue: *const c_void;
    static kCFBooleanFalse: *const c_void;
    static kCFAllocatorDefault: CFAllocatorRef;
}

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferCreate(
        allocator: CFAllocatorRef,
        width: usize,
        height: usize,
        pixel_format_type: u32,
        pixel_buffer_attributes: CFDictionaryRef,
        pixel_buffer_out: *mut CVPixelBufferRef,
    ) -> i32;
    fn CVPixelBufferLockBaseAddress(pb: CVPixelBufferRef, lock_flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pb: CVPixelBufferRef, unlock_flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddress(pb: CVPixelBufferRef) -> *mut c_void;
    fn CVPixelBufferGetBytesPerRow(pb: CVPixelBufferRef) -> usize;
    fn CVPixelBufferPoolCreate(
        allocator: CFAllocatorRef,
        pool_attributes: CFDictionaryRef,
        pixel_buffer_attributes: CFDictionaryRef,
        pool_out: *mut CVPixelBufferPoolRef,
    ) -> i32;
    fn CVPixelBufferPoolCreatePixelBuffer(
        allocator: CFAllocatorRef,
        pixel_buffer_pool: CVPixelBufferPoolRef,
        pixel_buffer_out: *mut CVPixelBufferRef,
    ) -> i32;
    static kCVPixelBufferPixelFormatTypeKey: CFStringRef;
    static kCVPixelBufferWidthKey: CFStringRef;
    static kCVPixelBufferHeightKey: CFStringRef;
    static kCVPixelBufferIOSurfacePropertiesKey: CFStringRef;
}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMTimeMake(value: i64, timescale: i32) -> CMTime;
    fn CMSampleBufferGetDataBuffer(sbuf: CMSampleBufferRef) -> CMBlockBufferRef;
    fn CMSampleBufferGetFormatDescription(sbuf: CMSampleBufferRef) -> CMFormatDescriptionRef;
    fn CMBlockBufferGetDataLength(buf: CMBlockBufferRef) -> usize;
    fn CMBlockBufferCopyDataBytes(
        buf: CMBlockBufferRef,
        offset: usize,
        data_length: usize,
        dest: *mut c_void,
    ) -> OSStatus;
    fn CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
        video_desc: CMFormatDescriptionRef,
        parameter_set_index: usize,
        parameter_set_pointer_out: *mut *const u8,
        parameter_set_size_out: *mut usize,
        parameter_set_count_out: *mut usize,
        nal_unit_header_length_out: *mut i32,
    ) -> OSStatus;
}

#[link(name = "VideoToolbox", kind = "framework")]
extern "C" {
    fn VTCompressionSessionCreate(
        allocator: CFAllocatorRef,
        width: i32,
        height: i32,
        codec_type: u32,
        encoder_specification: CFDictionaryRef,
        source_image_buffer_attributes: CFDictionaryRef,
        compressed_data_allocator: CFAllocatorRef,
        output_callback: Option<
            unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                OSStatus,
                VTEncodeInfoFlags,
                CMSampleBufferRef,
            ),
        >,
        output_callback_ref_con: *mut c_void,
        compression_session_out: *mut VTCompressionSessionRef,
    ) -> OSStatus;
    fn VTSessionSetProperty(
        session: *mut c_void,
        property_key: CFStringRef,
        property_value: CFTypeRef,
    ) -> OSStatus;
    fn VTCompressionSessionPrepareToEncodeFrames(session: VTCompressionSessionRef) -> OSStatus;
    fn VTCompressionSessionEncodeFrame(
        session: VTCompressionSessionRef,
        image_buffer: CVPixelBufferRef,
        presentation_time_stamp: CMTime,
        duration: CMTime,
        frame_properties: CFDictionaryRef,
        source_frame_refcon: *mut c_void,
        info_flags_out: *mut VTEncodeInfoFlags,
    ) -> OSStatus;
    fn VTCompressionSessionCompleteFrames(
        session: VTCompressionSessionRef,
        complete_until_presentation_time_stamp: CMTime,
    ) -> OSStatus;
    fn VTCompressionSessionInvalidate(session: VTCompressionSessionRef);

    static kVTCompressionPropertyKey_RealTime: CFStringRef;
    static kVTCompressionPropertyKey_AllowFrameReordering: CFStringRef;
    static kVTCompressionPropertyKey_AverageBitRate: CFStringRef;
    static kVTCompressionPropertyKey_ExpectedFrameRate: CFStringRef;
    static kVTCompressionPropertyKey_MaxKeyFrameInterval: CFStringRef;
    static kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration: CFStringRef;
    static kVTCompressionPropertyKey_ProfileLevel: CFStringRef;
    static kVTProfileLevel_H264_Main_AutoLevel: CFStringRef;
    static kVTEncodeFrameOptionKey_ForceKeyFrame: CFStringRef;
    static kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder: CFStringRef;
}

pub struct VtEncoder {
    session: VTCompressionSessionRef,
    pool: CVPixelBufferPoolRef,
    width: u32,
    height: u32,
    bitrate_bps: u32,
    fps: u32,
    /// Stay true until we emit a real IDR.
    need_idr: bool,
    frame_idx: i64,
    config_sent: bool,
    last_avcc: Option<Vec<u8>>,
    codec: String,
    slot: Box<CallbackSlot>,
}

struct CallbackSlot {
    pair: Mutex<Option<EncodeResult>>,
    cv: Condvar,
}

struct EncodeResult {
    status: OSStatus,
    info_flags: VTEncodeInfoFlags,
    data: Vec<u8>,
    avcc: Option<Vec<u8>>,
    codec: String,
}

impl VtEncoder {
    pub fn new(width: u32, height: u32, fps: u32, bitrate_bps: u32) -> Result<Self, H264Error> {
        let w = (width.max(2) & !1) as i32;
        let h = (height.max(2) & !1) as i32;
        let fps = fps.max(10).min(60);
        let bps = bitrate_bps.max(500_000).min(20_000_000);

        let mut slot = Box::new(CallbackSlot {
            pair: Mutex::new(None),
            cv: Condvar::new(),
        });
        let slot_ptr = (&mut *slot) as *mut CallbackSlot as *mut c_void;

        unsafe {
            let enc_spec = CFDictionaryCreateMutable(
                kCFAllocatorDefault,
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            CFDictionarySetValue(
                enc_spec,
                kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder as *const _,
                kCFBooleanTrue,
            );

            // Source attrs: BGRA + IOSurface (required for HW path on many Macs)
            let src_attrs = pixel_buffer_attrs(w as usize, h as usize);

            let mut session: VTCompressionSessionRef = ptr::null_mut();
            let st = VTCompressionSessionCreate(
                kCFAllocatorDefault,
                w,
                h,
                K_CM_VIDEO_CODEC_TYPE_H264,
                enc_spec as CFDictionaryRef,
                src_attrs as CFDictionaryRef,
                ptr::null(),
                Some(output_callback),
                slot_ptr,
                &mut session,
            );
            CFRelease(enc_spec as CFTypeRef);
            CFRelease(src_attrs as CFTypeRef);

            if st != 0 || session.is_null() {
                return Err(H264Error::OpenH264(format!(
                    "VTCompressionSessionCreate failed: {st}"
                )));
            }

            set_bool(session, kVTCompressionPropertyKey_RealTime, true);
            set_bool(
                session,
                kVTCompressionPropertyKey_AllowFrameReordering,
                false,
            );
            set_i32(
                session,
                kVTCompressionPropertyKey_AverageBitRate,
                bps as i32,
            );
            set_f64(
                session,
                kVTCompressionPropertyKey_ExpectedFrameRate,
                fps as f64,
            );
            let gop = (fps as i32).clamp(15, 60);
            set_i32(session, kVTCompressionPropertyKey_MaxKeyFrameInterval, gop);
            set_f64(
                session,
                kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration,
                1.0,
            );
            VTSessionSetProperty(
                session,
                kVTCompressionPropertyKey_ProfileLevel,
                kVTProfileLevel_H264_Main_AutoLevel as CFTypeRef,
            );

            let prep = VTCompressionSessionPrepareToEncodeFrames(session);
            if prep != 0 {
                VTCompressionSessionInvalidate(session);
                CFRelease(session as CFTypeRef);
                return Err(H264Error::OpenH264(format!(
                    "VTCompressionSessionPrepare failed: {prep}"
                )));
            }

            // Pixel buffer pool (reuse — avoids alloc thrash every frame)
            let pool_attrs = pixel_buffer_attrs(w as usize, h as usize);
            let mut pool: CVPixelBufferPoolRef = ptr::null_mut();
            let pst = CVPixelBufferPoolCreate(
                kCFAllocatorDefault,
                ptr::null(),
                pool_attrs as CFDictionaryRef,
                &mut pool,
            );
            CFRelease(pool_attrs as CFTypeRef);
            if pst != K_CV_RETURN_SUCCESS || pool.is_null() {
                // Fallback: session ok without pool
                warn!(pst, "CVPixelBufferPoolCreate failed — per-frame alloc");
            }

            debug!(w, h, bps, fps, "VideoToolbox session ready");
            Ok(Self {
                session,
                pool,
                width: w as u32,
                height: h as u32,
                bitrate_bps: bps,
                fps,
                need_idr: true,
                frame_idx: 0,
                config_sent: false,
                last_avcc: None,
                codec: "avc1.4D401F".into(),
                slot,
            })
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn bitrate_bps(&self) -> u32 {
        self.bitrate_bps
    }

    pub fn force_keyframe(&mut self) {
        self.need_idr = true;
        // Re-emit avcC with the next IDR so clients that tore down their
        // VideoDecoder (window switch) can reconfigure.
        self.config_sent = false;
    }

    pub fn encode_rgba(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        pts_us: i64,
    ) -> Result<H264Encoded, H264Error> {
        let w = (width.max(2) & !1) as usize;
        let h = (height.max(2) & !1) as usize;
        let expected = w * h * 4;
        if rgba.len() < expected {
            return Err(H264Error::OpenH264("rgba too short".into()));
        }
        if w as u32 != self.width || h as u32 != self.height {
            return Err(H264Error::OpenH264(format!(
                "size change {}x{} → {}x{}",
                self.width, self.height, w, h
            )));
        }

        let pb = unsafe { self.acquire_pixel_buffer()? };
        unsafe {
            fill_bgra_from_rgba(pb, w, h, &rgba[..expected])?;
        }

        let force = self.need_idr;
        {
            let mut g = self.slot.pair.lock().unwrap();
            *g = None;
        }

        // Microsecond clock — smoother than ms for 30fps.
        let pts = unsafe { CMTimeMake(pts_us.max(0), 1_000_000) };
        let duration = unsafe { CMTimeMake(1_000_000 / i64::from(self.fps.max(1)), 1_000_000) };

        let frame_props = if force {
            unsafe {
                let d = CFDictionaryCreateMutable(
                    kCFAllocatorDefault,
                    1,
                    &kCFTypeDictionaryKeyCallBacks,
                    &kCFTypeDictionaryValueCallBacks,
                );
                CFDictionarySetValue(
                    d,
                    kVTEncodeFrameOptionKey_ForceKeyFrame as *const _,
                    kCFBooleanTrue,
                );
                d as CFDictionaryRef
            }
        } else {
            ptr::null()
        };

        let mut flags: VTEncodeInfoFlags = 0;
        let st = unsafe {
            VTCompressionSessionEncodeFrame(
                self.session,
                pb,
                pts,
                duration,
                frame_props,
                ptr::null_mut(),
                &mut flags,
            )
        };
        unsafe {
            if !frame_props.is_null() {
                CFRelease(frame_props as CFTypeRef);
            }
            CFRelease(pb as CFTypeRef);
        }
        if st != 0 {
            return Err(H264Error::OpenH264(format!("VTEncodeFrame: {st}")));
        }

        // Wait for async callback (HW is usually <5ms).
        let result = {
            let mut g = self.slot.pair.lock().unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while g.is_none() {
                let now = std::time::Instant::now();
                if now >= deadline {
                    break;
                }
                let (gg, _) = self
                    .slot
                    .cv
                    .wait_timeout(g, deadline.saturating_duration_since(now))
                    .unwrap();
                g = gg;
            }
            g.take()
        };

        let Some(res) = result else {
            // Keep forcing IDR if we never got a sample.
            self.need_idr = true;
            return Err(H264Error::Empty);
        };
        if res.status != 0 {
            self.need_idr = true;
            return Err(H264Error::OpenH264(format!(
                "VT callback status {}",
                res.status
            )));
        }
        if (res.info_flags & K_VT_ENCODE_INFO_FRAME_DROPPED) != 0 || res.data.is_empty() {
            return Err(H264Error::Empty);
        }

        // WebCodecs wants VCL only when using avcC `description` (no in-band SPS/PPS/SEI).
        let vcl = strip_to_vcl_avcc(&res.data);
        if vcl.is_empty() {
            return Err(H264Error::Empty);
        }
        let has_idr = avcc_has_nal_type(&vcl, 5);
        if self.need_idr && !has_idr {
            self.need_idr = true;
            return Err(H264Error::Empty);
        }
        if has_idr {
            self.need_idr = false;
        }

        let mut avcc_config = None;
        if let Some(box_) = res.avcc {
            if self.last_avcc.as_ref() != Some(&box_) || !self.config_sent {
                self.last_avcc = Some(box_.clone());
                self.codec = res.codec.clone();
                self.config_sent = true;
                avcc_config = Some(box_);
            }
        }
        // First IDR must ship with avcC so the browser can configure VideoDecoder.
        if has_idr && avcc_config.is_none() && self.last_avcc.is_some() && !self.config_sent {
            avcc_config = self.last_avcc.clone();
            self.config_sent = true;
        }
        if has_idr && avcc_config.is_none() && self.last_avcc.is_none() {
            self.need_idr = true;
            return Err(H264Error::Empty);
        }

        self.frame_idx += 1;
        Ok(H264Encoded {
            avcc_au: vcl,
            keyframe: has_idr,
            pts_us,
            avcc_config,
            codec: self.codec.clone(),
            width: self.width,
            height: self.height,
        })
    }

    unsafe fn acquire_pixel_buffer(&self) -> Result<CVPixelBufferRef, H264Error> {
        let mut pb: CVPixelBufferRef = ptr::null_mut();
        if !self.pool.is_null() {
            let st = CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, self.pool, &mut pb);
            if st == K_CV_RETURN_SUCCESS && !pb.is_null() {
                return Ok(pb);
            }
        }
        let st = CVPixelBufferCreate(
            kCFAllocatorDefault,
            self.width as usize,
            self.height as usize,
            K_CV_PIXEL_FORMAT_TYPE_32_BGRA,
            ptr::null(),
            &mut pb,
        );
        if st != K_CV_RETURN_SUCCESS || pb.is_null() {
            return Err(H264Error::OpenH264(format!("CVPixelBufferCreate {st}")));
        }
        Ok(pb)
    }
}

impl Drop for VtEncoder {
    fn drop(&mut self) {
        unsafe {
            if !self.session.is_null() {
                let _ = VTCompressionSessionCompleteFrames(
                    self.session,
                    CMTime {
                        value: 0,
                        timescale: 0,
                        flags: 0,
                        epoch: 0,
                    },
                );
                VTCompressionSessionInvalidate(self.session);
                CFRelease(self.session as CFTypeRef);
                self.session = ptr::null_mut();
            }
            if !self.pool.is_null() {
                CFRelease(self.pool as CFTypeRef);
                self.pool = ptr::null_mut();
            }
        }
    }
}

unsafe extern "C" fn output_callback(
    output_callback_ref_con: *mut c_void,
    _source_frame_refcon: *mut c_void,
    status: OSStatus,
    info_flags: VTEncodeInfoFlags,
    sample_buffer: CMSampleBufferRef,
) {
    if output_callback_ref_con.is_null() {
        return;
    }
    let slot = &*(output_callback_ref_con as *const CallbackSlot);

    let mut data = Vec::new();
    let mut avcc = None;
    let mut codec = "avc1.4D401F".to_string();

    if status == 0 && !sample_buffer.is_null() && (info_flags & K_VT_ENCODE_INFO_FRAME_DROPPED) == 0
    {
        let block = CMSampleBufferGetDataBuffer(sample_buffer);
        if !block.is_null() {
            let len = CMBlockBufferGetDataLength(block);
            if len > 0 {
                data.resize(len, 0);
                let cs =
                    CMBlockBufferCopyDataBytes(block, 0, len, data.as_mut_ptr() as *mut c_void);
                if cs != K_CM_BLOCK_BUFFER_NO_ERR {
                    data.clear();
                }
            }
        }
        // VT H.264 sample buffers are already AVCC (4-byte length prefix).

        let fmt = CMSampleBufferGetFormatDescription(sample_buffer);
        if !fmt.is_null() {
            if let Some((box_, c)) = make_avcc_from_format(fmt) {
                avcc = Some(box_);
                codec = c;
            }
        }
    }

    let mut g = slot.pair.lock().unwrap();
    *g = Some(EncodeResult {
        status,
        info_flags,
        data,
        avcc,
        codec,
    });
    slot.cv.notify_one();
}

fn avcc_has_nal_type(data: &[u8], want: u8) -> bool {
    let mut i = 0;
    while i + 4 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        i += 4;
        if len == 0 || i + len > data.len() {
            break;
        }
        if (data[i] & 0x1f) == want {
            return true;
        }
        i += len;
    }
    false
}

/// Keep only VCL NALs (types 1 and 5) for WebCodecs + avcC description.
fn strip_to_vcl_avcc(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i + 4 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        i += 4;
        if len == 0 || i + len > data.len() {
            break;
        }
        let ntype = data[i] & 0x1f;
        if ntype == 1 || ntype == 5 {
            out.extend_from_slice(&(len as u32).to_be_bytes());
            out.extend_from_slice(&data[i..i + len]);
        }
        i += len;
    }
    out
}

unsafe fn make_avcc_from_format(fmt: CMFormatDescriptionRef) -> Option<(Vec<u8>, String)> {
    let mut sps_ptr: *const u8 = ptr::null();
    let mut sps_size: usize = 0;
    let mut pps_ptr: *const u8 = ptr::null();
    let mut pps_size: usize = 0;
    let mut nal_len: i32 = 0;

    let st = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
        fmt,
        0,
        &mut sps_ptr,
        &mut sps_size,
        ptr::null_mut(),
        &mut nal_len,
    );
    if st != 0 || sps_ptr.is_null() || sps_size < 4 {
        return None;
    }
    let st = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
        fmt,
        1,
        &mut pps_ptr,
        &mut pps_size,
        ptr::null_mut(),
        ptr::null_mut(),
    );
    if st != 0 || pps_ptr.is_null() || pps_size == 0 {
        return None;
    }
    let sps = std::slice::from_raw_parts(sps_ptr, sps_size).to_vec();
    let pps = std::slice::from_raw_parts(pps_ptr, pps_size).to_vec();
    let box_ = build_avcc(&sps, &pps);
    let codec = format!("avc1.{:02X}{:02X}{:02X}", sps[1], sps[2], sps[3]);
    Some((box_, codec))
}

unsafe fn pixel_buffer_attrs(w: usize, h: usize) -> CFMutableDictionaryRef {
    let d = CFDictionaryCreateMutable(
        kCFAllocatorDefault,
        4,
        &kCFTypeDictionaryKeyCallBacks,
        &kCFTypeDictionaryValueCallBacks,
    );
    let fmt = K_CV_PIXEL_FORMAT_TYPE_32_BGRA;
    let n_fmt = CFNumberCreate(
        kCFAllocatorDefault,
        K_CF_NUMBER_SINT32_TYPE,
        &fmt as *const _ as *const c_void,
    );
    let n_w = CFNumberCreate(
        kCFAllocatorDefault,
        K_CF_NUMBER_SINT32_TYPE,
        &(w as i32) as *const _ as *const c_void,
    );
    let n_h = CFNumberCreate(
        kCFAllocatorDefault,
        K_CF_NUMBER_SINT32_TYPE,
        &(h as i32) as *const _ as *const c_void,
    );
    let empty = CFDictionaryCreateMutable(
        kCFAllocatorDefault,
        0,
        &kCFTypeDictionaryKeyCallBacks,
        &kCFTypeDictionaryValueCallBacks,
    );
    CFDictionarySetValue(
        d,
        kCVPixelBufferPixelFormatTypeKey as *const _,
        n_fmt as *const _,
    );
    CFDictionarySetValue(d, kCVPixelBufferWidthKey as *const _, n_w as *const _);
    CFDictionarySetValue(d, kCVPixelBufferHeightKey as *const _, n_h as *const _);
    CFDictionarySetValue(
        d,
        kCVPixelBufferIOSurfacePropertiesKey as *const _,
        empty as *const _,
    );
    CFRelease(n_fmt as CFTypeRef);
    CFRelease(n_w as CFTypeRef);
    CFRelease(n_h as CFTypeRef);
    CFRelease(empty as CFTypeRef);
    d
}

unsafe fn fill_bgra_from_rgba(
    pb: CVPixelBufferRef,
    w: usize,
    h: usize,
    rgba: &[u8],
) -> Result<(), H264Error> {
    if CVPixelBufferLockBaseAddress(pb, 0) != 0 {
        return Err(H264Error::OpenH264("lock pixel buffer failed".into()));
    }
    let base = CVPixelBufferGetBaseAddress(pb) as *mut u8;
    let stride = CVPixelBufferGetBytesPerRow(pb);
    if base.is_null() {
        CVPixelBufferUnlockBaseAddress(pb, 0);
        return Err(H264Error::OpenH264("null base address".into()));
    }
    // Fast path: tight pack + swizzle R↔B
    for y in 0..h {
        let src = &rgba[y * w * 4..(y + 1) * w * 4];
        let dst = std::slice::from_raw_parts_mut(base.add(y * stride), w * 4);
        let mut x = 0;
        while x + 4 <= w * 4 {
            dst[x] = src[x + 2];
            dst[x + 1] = src[x + 1];
            dst[x + 2] = src[x];
            dst[x + 3] = src[x + 3];
            x += 4;
        }
    }
    CVPixelBufferUnlockBaseAddress(pb, 0);
    Ok(())
}

unsafe fn set_bool(session: VTCompressionSessionRef, key: CFStringRef, v: bool) {
    VTSessionSetProperty(
        session,
        key,
        if v { kCFBooleanTrue } else { kCFBooleanFalse } as CFTypeRef,
    );
}

unsafe fn set_i32(session: VTCompressionSessionRef, key: CFStringRef, v: i32) {
    let num = CFNumberCreate(
        kCFAllocatorDefault,
        K_CF_NUMBER_SINT32_TYPE,
        &v as *const _ as *const c_void,
    );
    if !num.is_null() {
        VTSessionSetProperty(session, key, num as CFTypeRef);
        CFRelease(num as CFTypeRef);
    }
}

unsafe fn set_f64(session: VTCompressionSessionRef, key: CFStringRef, v: f64) {
    let num = CFNumberCreate(
        kCFAllocatorDefault,
        K_CF_NUMBER_FLOAT64_TYPE,
        &v as *const _ as *const c_void,
    );
    if !num.is_null() {
        VTSessionSetProperty(session, key, num as CFTypeRef);
        CFRelease(num as CFTypeRef);
    }
}
