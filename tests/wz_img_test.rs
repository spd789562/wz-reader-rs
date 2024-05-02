use wz_reader::property::{WzValue, WzSubProperty, Vector2D};
use wz_reader::{WzNode, WzNodeArc, WzObjectType, node, wz_image, WzNodeCast,resolve_inlink};
use wz_reader::util;
use wz_reader::version::WzMapleVersion;

/**
 * the wz_img.img file structure:
 *   wz_img.img
 *     - conv
 *       - 0(vec)
 *       - 1(png)
 *         - origin
 *         - _inlink
 *     - 1
 *       - float
 *       - double
 *       - long
 *       - short
 *       - int
 *     - 2
 *       - nil
 *       - string
 *       - uol
 */

fn can_parse_node(node: &WzNodeArc) {
    let mut node_read = node.write().unwrap();

    assert!(node_read.parse(node).is_ok());
}

#[test]
fn should_parsing_with_default_version() {
    let wz_img = WzNode::from_img_file(r"tests/test.img", None, None);
    assert!(wz_img.is_ok());

    let wz_img = wz_img.unwrap().into_lock();
    
    can_parse_node(&wz_img);
}

#[test]
fn should_parsing_with_correct_version() {
    let wz_img = WzNode::from_img_file(r"tests/test.img", Some(WzMapleVersion::BMS), None);
    assert!(wz_img.is_ok());

    let wz_img = wz_img.unwrap().into_lock();
    
    can_parse_node(&wz_img);
}

#[test]
fn should_error_with_wrong_version() {
    let wz_img = WzNode::from_img_file(r"tests/test.img", Some(WzMapleVersion::EMS), None);
    assert!(wz_img.is_err());
}

#[test]
fn should_parsing_with_correct_iv() {
    let wz_img = WzNode::from_img_file_with_iv(r"tests/test.img", [0, 0, 0, 0], None);
    assert!(wz_img.is_ok());

    let wz_img = wz_img.unwrap().into_lock();
    
    can_parse_node(&wz_img);
}

#[test]
fn should_error_with_wrong_iv() {
    let wz_img = WzNode::from_img_file_with_iv(r"tests/test.img", [1, 2, 3, 4], None);
    assert!(wz_img.is_err());
}

fn check_sample_wz_img(wz_img: &WzNodeArc) {    
    can_parse_node(&wz_img);
    
    let wz_img_read = wz_img.read().unwrap();

    if let Some(first_folder) = wz_img_read.at("1") {
        let first_folder = first_folder.read().unwrap();

        assert!(matches!(first_folder.object_type, WzObjectType::Property(WzSubProperty::Property)));

        let int = first_folder.at("int");
        assert!(int.is_some());
        
        let int = int.unwrap();
        let int = int.read().unwrap();
        assert_eq!(int.try_as_int(), Some(&1));

        let short = first_folder.at("short");
        assert!(short.is_some());
        
        let short = short.unwrap();
        let short = short.read().unwrap();
        assert_eq!(short.try_as_short(), Some(&2));

        let long = first_folder.at("long");
        assert!(long.is_some());
        
        let long = long.unwrap();
        let long = long.read().unwrap();
        assert_eq!(long.try_as_long(), Some(&3));

        let float = first_folder.at("float");
        assert!(float.is_some());
        
        let float = float.unwrap();
        let float = float.read().unwrap();
        assert_eq!(float.try_as_float(), Some(&4.1));

        let double = first_folder.at("double");
        assert!(double.is_some());
        
        let double = double.unwrap();
        let double = double.read().unwrap();
        assert_eq!(double.try_as_double(), Some(&4.2));
    }

    if let Some(second_folder) = wz_img_read.at("2") {
        let second_folder = second_folder.read().unwrap();

        assert!(matches!(second_folder.object_type, WzObjectType::Property(WzSubProperty::Property)));

        let nil = second_folder.at("nil");
        assert!(nil.is_some());
        
        let nil = nil.unwrap();
        let nil = nil.read().unwrap();
        assert!(matches!(nil.object_type, WzObjectType::Value(WzValue::Null)));

        let string = second_folder.at("string");
        assert!(string.is_some());
        
        let string = string.unwrap();
        let string = string.read().unwrap();
        let string = string.try_as_string();
        if let Some(string) = string {
            let s = string.get_string();
            assert!(s.is_ok());
            assert_eq!(&s.unwrap(), "foo");
        }

        let uol = second_folder.at("uol");
        assert!(uol.is_some());
        
        let uol = uol.unwrap();
        let uol = uol.read().unwrap();
        if let WzObjectType::Value(WzValue::UOL(string)) = &uol.object_type {
            let s = string.get_string();
            assert!(s.is_ok());
            assert_eq!(&s.unwrap(), "string");
        }
    }

    if let Some(convex_folder) = wz_img_read.at("conv") {
        let convex_folder = convex_folder.read().unwrap();

        assert!(matches!(convex_folder.object_type, WzObjectType::Property(WzSubProperty::Convex)));

        let vector = convex_folder.at("0");
        assert!(vector.is_some());
        
        let vector = vector.unwrap();
        let vector = vector.read().unwrap();
        if let WzObjectType::Value(WzValue::Vector(vec)) = &vector.object_type {
            assert_eq!(vec, &Vector2D(1, 1));
        }

        let png = convex_folder.at("1");
        assert!(png.is_some());
        
        let png = png.unwrap();
        let png = png.read().unwrap();
        assert!(matches!(png.object_type, WzObjectType::Property(WzSubProperty::PNG(_))));

        let vector = png.at("origin");
        assert!(vector.is_some());

        let vector = vector.unwrap();
        let vector = vector.read().unwrap();
        if let WzObjectType::Value(WzValue::Vector(vec)) = &vector.object_type {
            assert_eq!(vec, &Vector2D(0, 0));
        }

        let inlink = png.at("_inlink");
        assert!(inlink.is_some());

        let inlink = inlink.unwrap();
        let inlink_str = if let WzObjectType::Value(WzValue::String(string)) = &inlink.read().unwrap().object_type {
            string.get_string().unwrap()
        } else {
            String::new()
        };
        assert_eq!(&inlink_str, "conv/png2");
    }
}

#[test]
fn should_parsing_wz_img_and_check_values() {
    let wz_file = WzNode::from_wz_file(r"tests/test.wz", Some(WzMapleVersion::BMS), Some(123), None);
    assert!(wz_file.is_ok());

    let wz_file = wz_file.unwrap().into_lock();
    
    assert!(wz_file.write().unwrap().parse(&wz_file).is_ok());

    let wz_file_read = wz_file.read().unwrap();

    let wz_img = wz_file_read.at("wz_img.img");
    assert!(wz_img.is_some());

    check_sample_wz_img(&wz_img.unwrap());
}

#[test]
fn should_success_using_wz_node_methods_on_childs() {
    let wz_img = WzNode::from_img_file(r"tests/test.img", Some(WzMapleVersion::BMS), None);
    assert!(wz_img.is_ok());

    let wz_img = wz_img.unwrap().into_lock();

    assert!(wz_img.write().unwrap().parse(&wz_img).is_ok());

    let nil_node = wz_img.read().unwrap().at_path("2/nil");
    assert!(nil_node.is_some());

    let wz_node_not_exist = wz_img.read().unwrap().at_path("2/not_exist");
    assert!(wz_node_not_exist.is_none());

    let nil_node = nil_node.unwrap();
    let nil_read = nil_node.read().unwrap();
    assert_eq!(nil_read.get_full_path(), "test.img/2/nil");

    let nil_parent = nil_read.at_relative("..");
    assert!(nil_parent.is_some());
    assert_eq!(nil_parent.unwrap().read().unwrap().get_full_path(), "test.img/2");

    let vec_node = nil_read.at_path_relative("../../conv/0");
    assert!(vec_node.is_some());
    let vec_node = vec_node.unwrap();
    assert_eq!(vec_node.read().unwrap().get_full_path(), "test.img/conv/0");

    let png_node = nil_read.at_path_relative("../../conv/1");
    assert!(png_node.is_some());
    let png_node = png_node.unwrap();
    assert_eq!(png_node.read().unwrap().get_full_path(), "test.img/conv/1");

    let wz_node_not_exist = nil_read.at_path_relative("../../not_exist");
    assert!(wz_node_not_exist.is_none());

    let inlink = png_node.read().unwrap().at("_inlink");
    assert!(inlink.is_some());
    let inlink = inlink.unwrap();
    let inlink_read = inlink.read().unwrap();
    let inlink_string = inlink_read.try_as_string();

    assert!(inlink_string.is_some());
    let inlink_string = inlink_string.unwrap().get_string();

    assert!(inlink_string.is_ok());
    
    let inlink_string = inlink_string.unwrap();
    let inlink_target = resolve_inlink(&inlink_string, &inlink);
    assert!(inlink_target.is_none());

    let parent_img = png_node.read().unwrap().get_parent_wz_image();
    assert!(parent_img.is_some());
    let parent_img = parent_img.unwrap();
    assert_eq!(parent_img.read().unwrap().get_full_path(), wz_img.read().unwrap().get_full_path());

    let force_get_next_exist_node = parent_img.read().unwrap().at_path_parsed("2/not_exist");
    assert!(matches!(force_get_next_exist_node, Err(node::Error::NodeNotFound)));

    let force_get_some_node = parent_img.read().unwrap().at_path_parsed("2/nil");
    assert!(force_get_some_node.is_ok());

    parent_img.write().unwrap().unparse();

    assert_eq!(parent_img.read().unwrap().children.len(), 0);

    let parent_img_read = parent_img.read().unwrap();
    if let WzObjectType::Image(wz_image) = &parent_img_read.object_type {
        let direct_access_not_exist = wz_image.at_path("2/not_exist");

        assert!(matches!(direct_access_not_exist, Err(wz_image::Error::ParsePropertyListError(util::WzPropertyParseError::NodeNotFound))));

        let direct_access_nil = wz_image.at_path("2/nil");
        assert!(direct_access_nil.is_ok());

        let nil = direct_access_nil.unwrap();
        let nil = nil.read().unwrap();
        assert!(matches!(nil.object_type, WzObjectType::Value(WzValue::Null)));
    }
}

#[test]
fn should_success_walk_thorugh() {
    let wz_img = WzNode::from_img_file(r"tests/test.img", Some(WzMapleVersion::BMS), None);
    assert!(wz_img.is_ok());

    let wz_img = wz_img.unwrap().into_lock();
    
    let pathes = std::collections::HashSet::from([
        "test.img",
        "test.img/conv",
        "test.img/conv/0",
        "test.img/conv/1",
        "test.img/conv/1/_inlink",
        "test.img/conv/1/origin",
        "test.img/1",
        "test.img/1/float",
        "test.img/1/double",
        "test.img/1/long",
        "test.img/1/short",
        "test.img/1/int",
        "test.img/2",
        "test.img/2/nil",
        "test.img/2/string",
        "test.img/2/uol",
    ]);

    util::walk_node(&wz_img, true, &|node| {
        let node_read = node.read().unwrap();
        println!("{}", node_read.get_full_path());
        assert!(pathes.contains(node_read.get_full_path().as_str()));
    });
}