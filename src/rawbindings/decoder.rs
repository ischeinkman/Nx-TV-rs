use decoder_shim::{VideoDecoder, Codec, Frame};
use super::ffmpeg_ffi::{
    lang_items, 
    AVFrame, AVFormatContext, AVCodecContext, AVCodec, AVStream, AVPacket, 
    AVCodecParameters, AVMediaType, AVMediaType_AVMEDIA_TYPE_VIDEO,
    AVPixelFormat_AV_PIX_FMT_YUV420P, AVPixelFormat_AV_PIX_FMT_RGB24,
    AVPixelFormat_AV_PIX_FMT_RGBA64BE, AVPixelFormat_AV_PIX_FMT_ARGB, 
    AVPixelFormat_AV_PIX_FMT_BGR24,
    AVPixelFormat_AV_PIX_FMT_RGBA, 
    SWS_BILINEAR, 
    SwsContext,
    av_frame_alloc, av_frame_free, 
    av_packet_alloc, 
    av_register_all, avcodec_register_all,
    av_log_set_level, AV_LOG_QUIET, 
    avformat_open_input, avformat_find_stream_info, 
    av_dump_format, avcodec_find_decoder, avcodec_alloc_context3, 
    avcodec_parameters_to_context, avcodec_open2, av_read_frame, 
    avcodec_send_packet, avcodec_receive_frame, sws_getContext, 
    av_image_alloc, sws_scale, av_freep,
    avformat_close_input,
    av_packet_unref,
    av_frame_unref,
    avcodec_free_context,
    avformat_free_context,
};
use std::path::Path;
use std::path::PathBuf;
use std::ffi::{CString, CStr};
use std::ptr;
use std::slice;

#[derive(Debug)]
pub struct RawFfmpegDecoder {
    ctx_format : *mut AVFormatContext, 
    ctx_codec : *mut AVCodecContext, 
    codec : *mut AVCodec, 
    frame : *mut AVFrame, 
    rgbframe : *mut AVFrame, 
    stream_idx : usize, 
    ctx_sws : *mut SwsContext, 
    pkt : *mut AVPacket, 
    counter : usize,
}

impl RawFfmpegDecoder {
    pub fn new<T : AsRef<Path>>(path : T) -> Result<RawFfmpegDecoder, String> {
        unsafe {
            let mut frame = av_frame_alloc();
            let mut rgbframe = av_frame_alloc();
            let mut pkt = av_packet_alloc();

            av_register_all();
            avcodec_register_all();
            av_log_set_level(AV_LOG_QUIET);
            
            let path_bytes : String = path
                .as_ref()
                .to_str()
                .ok_or("Could not get path &str!".to_owned())
                .map(|path_str| 
                    format!("{}\0", path_str).to_owned()
                )?;
            let mut ctx_format : *mut AVFormatContext = ptr::null_mut();
            let open_errno = avformat_open_input(&mut ctx_format as *mut *mut AVFormatContext, path_bytes.as_ptr(), ptr::null_mut(), ptr::null_mut());
            if open_errno !=0 {
                return Err(format!("Could not open input. Errno: {}", open_errno).to_owned());
            }

            let find_stream_errno = avformat_find_stream_info(ctx_format, ptr::null_mut());
            if find_stream_errno <0 {
                return Err(format!("Could not find stream info. Errno: {}", find_stream_errno).to_owned());
            }

            av_dump_format(ctx_format, 0, path_bytes.as_ptr(), 0);
            let num_streams = (*ctx_format).nb_streams;
            let (stream_idx, vid_stream) = (0usize..num_streams as usize).find_map(|idx| {
                let stream_ptr = (*ctx_format).streams.offset(idx as isize);
                let stream_raw = *stream_ptr;
                if (*(*stream_raw).codecpar).codec_type == AVMediaType_AVMEDIA_TYPE_VIDEO {
                    Some((idx, stream_ptr))
                }
                else {
                    None
                }
            }).ok_or("Error getting video stream.")?;

            let codec = avcodec_find_decoder((*(**vid_stream).codecpar).codec_id);
            if codec.is_null() {
                return Err("Error finding a decoder (strange).".to_owned());
            }

            let mut ctx_codec = avcodec_alloc_context3(codec);
            if ctx_codec.is_null() {
                return Err("Error allocating ctx_codec.".to_owned());
            }
            let param_to_ctx_errno = avcodec_parameters_to_context(ctx_codec, (**vid_stream).codecpar);
            if param_to_ctx_errno <0 {
                return Err(format!("Error sending parameters to codec context: {}.", param_to_ctx_errno).to_owned());
            }

            let codec_open_errno = avcodec_open2(ctx_codec, codec, ptr::null_mut());
            if codec_open_errno < 0 {
                return Err(format!("Error opening codec with context: {}.", codec_open_errno).to_owned())
            }

            Ok(RawFfmpegDecoder {
                ctx_format,
                ctx_codec,
                codec,
                frame,
                rgbframe,
                stream_idx,
                ctx_sws : ptr::null_mut(),
                pkt,
                counter : 0,
            })
        }
    }

    unsafe fn next_frame(&mut self) -> Option<FfmpegFrame> {
        //eprintln!("next_frame begin.");
        //eprintln!("Reading frame data.");
        let mut has_frames_left = av_read_frame(self.ctx_format, self.pkt);
        //eprintln!("Got {} from av_read_frame.", has_frames_left);
        if has_frames_left < 0 {
            eprintln!("SS1: {} frames left, pkt stream {} vs {}.", has_frames_left, (*self.pkt).stream_index, self.stream_idx);
            return None;
        }
        while (*self.pkt).stream_index < 0 || (*self.pkt).stream_index as usize != self.stream_idx {
            has_frames_left = av_read_frame(self.ctx_format, self.pkt);
            if has_frames_left < 0 {
                eprintln!("SS1: {} frames left, pkt stream {} vs {}.", has_frames_left, (*self.pkt).stream_index, self.stream_idx);
                return None;
            }
        }

        //eprintln!("Sending packet.");
        let snd_pkt_errno = avcodec_send_packet(self.ctx_codec, self.pkt);
        //eprintln!("Got snd_pkt_errno of {}.", snd_pkt_errno);
        if snd_pkt_errno < 0 {
            eprintln!("SS2");
            return None;
        }
        //eprintln!("Recieving frame.");
        let mut rcv_frame_errno = avcodec_receive_frame(self.ctx_codec, self.frame);
        for _idx in 0..10 {
            //eprintln!("Got rcv_frame_errno of {}.", rcv_frame_errno);
            if rcv_frame_errno != -11 {
                break;
            }
            av_read_frame(self.ctx_format, self.pkt);
            avcodec_send_packet(self.ctx_codec, self.pkt);
            rcv_frame_errno = avcodec_receive_frame(self.ctx_codec, self.frame);
        }
        if rcv_frame_errno < 0 {
            eprintln!("SS3");
            return None;
        }
        //eprintln!("Getting context.");
        let frame : &mut AVFrame = &mut *self.frame;
        let frame_dt : *const *const u8 = &frame.data[0] as *const *mut u8 as *const *const u8;
        let frame_lns : *mut i32 = &mut frame.linesize[0] as * mut i32;
        let mut rgbframe : &mut AVFrame = &mut *self.rgbframe;
        let mut rgbframe_dt : *mut *mut u8 = &mut rgbframe.data[0] as *mut *mut u8;
        let rgbframe_lns : *mut i32 = &mut rgbframe.linesize[0] as * mut i32;
        let fw = (*self.frame).width;
        let fh = (*self.frame).height;
        self.ctx_sws = sws_getContext(fw, fh, AVPixelFormat_AV_PIX_FMT_YUV420P, fw, fh, AVPixelFormat_AV_PIX_FMT_ARGB, SWS_BILINEAR as i32, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
        
        //eprintln!("Entering final strech.");
        rgbframe.width = fw;
        rgbframe.height = fh;
        rgbframe.format = AVPixelFormat_AV_PIX_FMT_ARGB;
        av_image_alloc(rgbframe_dt, rgbframe_lns, rgbframe.width, rgbframe.height, rgbframe.format, 32);
        sws_scale(self.ctx_sws, frame_dt, frame_lns, 0, fh, rgbframe_dt, rgbframe_lns);
        //eprintln!("Finished next_frame");
        FfmpegFrame::new(rgbframe.width as usize, rgbframe.height as usize, &rgbframe.data, 0u64).ok()

    }
}

impl Drop for RawFfmpegDecoder {
    fn drop(&mut self) {
        /*unsafe {
            //eprintln!("Entered RawFfmpegDecoder::drop"); 
            //eprintln!("Self = \n{:?}\n", self);
            //eprintln!("Cleaning format input.");
            if !self.ctx_format.is_null(){
                avformat_close_input(&mut self.ctx_format as *mut *mut AVFormatContext);
            }
            //eprintln!("Cleaning pkt.");
            if !self.pkt.is_null() {
                av_packet_unref(self.pkt);
            }
            //eprintln!("Cleaning rgbframe");
            if !self.rgbframe.is_null() {
                av_frame_unref(self.rgbframe);
            }
            //eprintln!("Cleaning frame");
            if !self.frame.is_null() {
                av_frame_unref(self.frame );
            }
            //eprintln!("Cleaning ctx_codec");
            if !self.ctx_codec.is_null(){
                let mut ctx_codec_ptr : *mut AVCodecContext = self.ctx_codec;
                //eprintln!("Going to try to clean ctx_codec_ptr: {:?} -> {:?}", &mut ctx_codec_ptr as *mut *mut AVCodecContext, ctx_codec_ptr);
                let vl = *ctx_codec_ptr;
                //eprintln!("Vl: {:?}", vl);
                avcodec_free_context(&mut ctx_codec_ptr as *mut *mut AVCodecContext);
            }
            //eprintln!("Cleaning ctx_format");
            if !self.ctx_format.is_null(){
                avformat_free_context(self.ctx_format);
            }
            //eprint!("Left RawFfmpegDecoder::drop"); 
        }*/
    }
}

impl Iterator for RawFfmpegDecoder {
    type Item = FfmpegFrame;

    fn next(&mut self) -> Option<FfmpegFrame> {
        unsafe { self.next_frame()}
    }
}

impl VideoDecoder for RawFfmpegDecoder {

    fn video_width(&self) -> usize {
        //TODO: Pre-calculate
        unsafe { (*self.frame).width as usize }
    }

    fn video_height(&self) -> usize {
        //TODO: Pre-calculate
        unsafe { (*self.frame).height as usize }
    }

    fn source(&self) -> &str {
        unsafe {
            CStr::from_ptr((*self.ctx_format).url)
                .to_str()
                .unwrap_or("ERROR GETTING SOURCE!")
        }
    }

    fn codec(&self) -> Codec {
        unsafe {
            Codec {
                name : CStr::from_ptr((*self.codec).name)
                    .to_str()
                    .unwrap_or("ERROR GETTING CODEC!")
                    .to_owned()
            }
        }
    }
}


pub struct FfmpegFrame {
    width : usize, 
    height : usize, 
    av_data_ptr : [*mut u8 ; 8usize], //Emulate the ffmpeg type
    nanos_from_prev : u64
}

impl FfmpegFrame {
    fn new(width : usize, height : usize, pixel_data_ptr : &[*mut u8 ; 8usize], nanos_from_prev : u64) -> Result<FfmpegFrame, String> {
        let arr_cpy : [*mut u8 ; 8usize] = [
            pixel_data_ptr[0],
            pixel_data_ptr[1],
            pixel_data_ptr[2],
            pixel_data_ptr[3],
            pixel_data_ptr[4],
            pixel_data_ptr[5],
            pixel_data_ptr[6],
            pixel_data_ptr[7]
        ];
        
        Ok(FfmpegFrame{
            width, 
            height, 
            av_data_ptr : arr_cpy, 
            nanos_from_prev
        })
    }
}

impl Drop for FfmpegFrame {
    fn drop(&mut self) {
        unsafe {
            //eprintln!("Entered FfmpegFrame::drop");
            //eprintln!("Cleaning av_data_ptr");
            if !self.av_data_ptr[0].is_null(){
                av_freep(&mut self.av_data_ptr[0] as *mut *mut u8 as *mut lang_items::c_void);
            }
            //eprintln!("Left FfmpegFrame::drop");
        }
    }
}

impl Frame for FfmpegFrame {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn rgba_buff(&self) -> &[u8] {
        unsafe { 
            slice::from_raw_parts(self.av_data_ptr[0], self.width * self.height * 4)
        }
    }

    fn nanos_from_prev(&self) -> u64 {
        self.nanos_from_prev
    }
}