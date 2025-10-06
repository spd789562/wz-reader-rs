
#[inline]
pub fn sum_str(string: &str) -> usize {
  string.as_bytes().iter().map(|&b| b as usize).sum::<usize>()
}

#[inline]
pub fn get_ascii_file_name<P>(file_name: P)-> String
where
  P: AsRef<std::path::Path>,
{
  file_name.as_ref().file_name().unwrap().to_str().unwrap().to_ascii_lowercase()
}
