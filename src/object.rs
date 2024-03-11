#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WzObjectType {
    File,
    Image,
    Directory,
    Property,
    List
}