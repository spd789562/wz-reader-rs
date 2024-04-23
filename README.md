# wz-reader-rs
Maplestory *.Wz file reading written in rust, try port similer code from [WzComparerR2.WzLib](https://github.com/Kagamia/WzComparerR2/tree/master/WzComparerR2.WzLib) and [MapleLib](https://github.com/lastbattle/MapleLib)

It a rust learning project, performance maybe not as good as C#.

## Dependencies
  - Image
    * flate2
    * image
  - Char Decryption
    * aes
    * ecb
  - Data
    * hashbrown - Hashmap
    * memmap2
  - Others
    * rayon
    * scroll
    * thiserror

## Minimum supported Rust version

wz_reader's MSRV is 1.70.0

## Example
```rust
use wz_reader::util::{resolve_base, walk_node};
// NodeCast trait provide try_as_* functions to casting WzNode
use wz_reader::NodeCast;

fn main() {
    // resolve wz files
    let base_node = resolve_base(r"D:\MapleStory\Data\Base.wz", None).unwrap();

    // try to parsing every nodes on the way
    walk_node(&base_node, true, &|node| {
        let node_read = node.read().unwrap();

        if let Some(sound_node) = node_read.try_as_sound() {
            let path = std::path::Path::new("./sounds").join(node_read.name.as_str());
            if sound_node.extract_sound(path).is_err() {
                println!("failed to extract sound: {}", node_read.get_full_path());
            }
        }
    })
}
```

You can find more example usage in the [examples](./examples) folder.

## License

This project is licensed under the [MIT license](./LICENSE.txt).