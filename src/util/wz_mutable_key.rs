use crate::util::string_decryptor;

#[deprecated(
    since = "0.18.0",
    note = "use util::string_decryptor::EcbDecryptor directly"
)]
pub type WzMutableKey = string_decryptor::EcbDecryptor;
