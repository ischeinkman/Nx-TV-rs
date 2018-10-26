use std::iter::Iterator;

pub trait Frame {
    fn width(&self) -> usize;

    fn height(&self) -> usize; 

    fn rgba_buff(&self) -> &[u8];

    fn nanos_from_prev(&self) -> u64;
}

pub trait VideoDecoder where Self : Iterator, <Self as Iterator>::Item : Frame{

    fn video_width(&self) -> usize;

    fn video_height(&self) -> usize;

    fn source(&self) -> &str ;

    fn codec(&self) -> Codec ;

}

pub struct Codec {
    pub name : String
}