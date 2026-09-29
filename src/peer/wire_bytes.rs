pub trait WireBytes {
    fn write_to(&self, out: &mut impl std::io::Write) -> std::io::Result<()>;
}

impl WireBytes for Vec<u8> {
    fn write_to(&self, out: &mut impl std::io::Write) -> std::io::Result<()> {
        out.write_all(self)
    }
}

impl<const N: usize> WireBytes for [u8; N] {
    fn write_to(&self, out: &mut impl std::io::Write) -> std::io::Result<()> {
        out.write_all(self)
    }
}
