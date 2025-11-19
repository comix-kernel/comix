use super::Driver;

pub trait RtcDriver: Driver {
    // read seconds since epoch
    fn read_epoch(&self) -> u64;
}
