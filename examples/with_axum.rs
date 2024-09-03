use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use image::ImageFormat;
use serde::Deserialize;
use serde_json::Value;
use std::io::{BufWriter, Cursor};
use std::sync::{Arc, Mutex, RwLock};
use wz_reader::{
    node, property,
    util::{node_util, resolve_base, walk_node},
    version::WzMapleVersion,
    WzNodeArc, WzNodeCast, WzNodeName,
};

#[derive(Clone)]
pub struct ServerState {
    pub wz_root: Arc<RwLock<Option<WzNodeArc>>>,
}

// run example with `cargo run --package wz_reader --example with_axum --features "json image/default-formats"`
// and open 127.0.0.1:3000 in your browser
#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let default_port = "3000".to_string();
    let port = args.get(1).unwrap_or(&default_port);

    let state = ServerState {
        wz_root: Arc::new(RwLock::new(None)),
    };

    let app = Router::new()
        .route("/", get(handler))
        .route("/init_wz_root", post(init_wz_root))
        .route("/get_json/*path", get(get_json))
        .route("/get_image/*path", get(get_image))
        .route("/get_image_urls/*path", get(get_image_urls))
        .route("/get_sound/*path", get(get_sound))
        .route("/browse", get(simple_browse_root))
        .route("/browse/*path", get(simple_browse))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>This is a Axum example for wz_reader</h1> \
        <p>Try to do <b>post</b> on <b>/init_wz_root</b> first</p>
        <p>Something like</p>
        <pre>fetch('http://127.0.0.1:3000/init_wz_root',{method: 'post', body: JSON.stringify({path:'D:\\path\\to\\Data\\Base\\Base.wz', version: 'BMS'}), headers: {'Content-Type':'Application/json'}})</pre>
        <p>or you can modify the code to initialize wz root use fixed path.</p>
        <p>Then you can do `get` on `/get_json` or `/get_image` or `/get_image_urls` or `/get_sound`</p>
        <ul>
            <li><a href=\"/get_json/Etc/BossLucid.img/Butterfly?force_parse=true&simple=true\" target=\"_blank\">/get_json/Etc/BossLucid.img/Butterfly?force_parse=true&simple=true</a></li>
            <li><a href=\"/get_json/Etc/BossLucid.img/Butterfly?force_parse=true&simple=false\" target=\"_blank\">/get_json/Etc/BossLucid.img/Butterfly?force_parse=true&simple=false</a></li>
            <li><a href=\"/get_image_urls/Etc/BossLucid.img/Butterfly?force_parse=true\" target=\"_blank\">/get_image_urls/Etc/BossLucid.img/Butterfly?force_parse=true</a></li>
            <li><a href=\"/get_image/Etc/BossLucid.img/Butterfly/butterfly/0/fly/0?force_parse=true\" target=\"_blank\">/get_image/Etc/BossLucid.img/Butterfly/butterfly/0/fly/0?force_parse=true</a></li>
            <li><a href=\"/get_sound/Sound/Bgm00.img/SleepyWood?force_parse=true\" target=\"_blank\">/get_sound/Etc/BossLucid.img/Butterfly?force_parse=true</a></li>
        </ul>
        <p>You can even access a simple browse on <a href=\"/browse\" target=\"_blank\">/browse</a></p>
        "
    )
}

/* init wz part */
enum InitWzError {
    MissingParam,
    ParseError,
    VersionError,
    IoError,
}
impl IntoResponse for InitWzError {
    fn into_response(self) -> Response {
        match self {
            InitWzError::MissingParam => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("should passing path and version".into())
                .unwrap(),
            InitWzError::ParseError => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("wz parse error".into())
                .unwrap(),
            InitWzError::VersionError => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("passing wrong wz version".into())
                .unwrap(),
            InitWzError::IoError => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("file error".into())
                .unwrap(),
        }
    }
}
async fn init_wz_root(
    State(ServerState { wz_root }): State<ServerState>,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse, InitWzError> {
    let base_path = body.get("path").and_then(|v| v.as_str());
    let version = body.get("version").and_then(|v| v.as_str());

    if base_path.is_none() {
        return Err(InitWzError::MissingParam);
    }

    let version = match version.unwrap_or_default() {
        "BMS" => Some(WzMapleVersion::BMS),
        "GMS" => Some(WzMapleVersion::GMS),
        "EMS" => Some(WzMapleVersion::EMS),
        _ => None,
    };

    let base_path = base_path.unwrap();

    if base_path.is_empty() {
        return Err(InitWzError::MissingParam);
    }

    let result = resolve_base(base_path, version);
    if result.is_err() {
        return Err(InitWzError::IoError);
    }
    let base_node = result.unwrap();
    let mut wz_root = wz_root.write().unwrap();
    *wz_root = Some(base_node);

    return Ok(StatusCode::OK);
}

#[derive(Debug)]
enum NodeFindError {
    Uninitialized,
    NotFound,
    TypeMismatch,
    ServerError,
    ParseError,
}
impl IntoResponse for NodeFindError {
    fn into_response(self) -> Response {
        match self {
            NodeFindError::Uninitialized => (
                StatusCode::BAD_REQUEST,
                "wz uninitialized, please do `/init_wz_root` first",
            )
                .into_response(),
            NodeFindError::NotFound => (StatusCode::NOT_FOUND, "node not found").into_response(),
            NodeFindError::TypeMismatch => {
                (StatusCode::BAD_REQUEST, "node type can't use on this route").into_response()
            }
            NodeFindError::ServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "something wrong when parsing data",
            )
                .into_response(),
            NodeFindError::ParseError => {
                (StatusCode::BAD_REQUEST, "node parse error").into_response()
            }
        }
    }
}
impl From<node::Error> for NodeFindError {
    fn from(e: node::Error) -> Self {
        match e {
            node::Error::NodeNotFound => NodeFindError::NotFound,
            _ => NodeFindError::ParseError,
        }
    }
}

fn get_node_from_root(
    root: Arc<RwLock<Option<WzNodeArc>>>,
    path: &str,
    force_parse: bool,
) -> Result<WzNodeArc, NodeFindError> {
    let wz_root = root.read().unwrap();

    if wz_root.is_none() {
        return Err(NodeFindError::Uninitialized);
    }

    let wz_root = wz_root.as_ref().unwrap();

    if path.is_empty() {
        return Ok(wz_root.clone());
    }

    let target = if force_parse {
        wz_root.at_path_parsed(&path)?
    } else {
        wz_root.at_path(&path).ok_or(node::Error::NodeNotFound)?
    };

    if force_parse {
        node_util::parse_node(&target)?;
    }

    Ok(target)
}

/* grabe json part */
#[derive(Deserialize)]
struct GetJsonParam {
    simple: Option<bool>,
    force_parse: Option<bool>,
    sort: Option<bool>,
}

async fn get_json(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Result<impl IntoResponse, NodeFindError> {
    println!("try to get path's json: {}", path);
    let is_simple = param.simple.unwrap_or(false);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = get_node_from_root(wz_root, &path, force_parse)?;

    let json = if is_simple {
        target.to_simple_json()
    } else {
        target.to_json()
    };

    let json = json.map_err(|_| NodeFindError::ServerError)?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json;charset=utf-8")],
        Body::from(json.to_string()),
    ))
}

/* grabe image part */
async fn get_image(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Result<impl IntoResponse, NodeFindError> {
    println!("try to get image: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = get_node_from_root(wz_root, &path, force_parse)?;

    if let Some(_) = target.try_as_png() {
        let img = property::get_image(&target).map_err(|_| NodeFindError::ServerError)?;

        let mut buf = BufWriter::new(Cursor::new(Vec::new()));
        // maybe use ImageFormat::Webp is better it quicker and smaller.
        img.write_to(&mut buf, ImageFormat::Bmp)
            .map_err(|_| NodeFindError::ServerError)?;

        let body = Body::from(buf.into_inner().unwrap().into_inner());

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/bmp")
            .header(header::CACHE_CONTROL, "max-age=3600")
            .body(body)
            .map_err(|_| NodeFindError::ServerError);
    }

    return Err(NodeFindError::TypeMismatch);
}

/* grabe image urls part */
async fn get_image_urls(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Result<impl IntoResponse, NodeFindError> {
    println!("try to get images from a node: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = get_node_from_root(wz_root, &path, force_parse)?;

    let urls = Mutex::new(Vec::new());

    walk_node(&target, force_parse, &|node| {
        if let Some(_) = node.try_as_png() {
            let path = node.get_full_path().replace("Base/", "");
            let mut urls = urls.lock().unwrap();
            urls.push(path);
        }
    });

    let urls = urls.into_inner().unwrap();

    let json = serde_json::to_string(&urls).unwrap();

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json;charset=utf-8")],
        Body::from(json),
    ))
}

/* grabe sound part */
async fn get_sound(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Result<impl IntoResponse, NodeFindError> {
    println!("try to get sound: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = get_node_from_root(wz_root, &path, force_parse)?;

    if let Some(sound) = target.try_as_sound() {
        let sound_buf = sound.get_buffer();

        let mini = match sound.sound_type {
            property::WzSoundType::Wav => "audio/wav",
            _ => "audio/mpeg",
        };

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mini)
            .body::<Body>(sound_buf.into())
            .map_err(|_| NodeFindError::ServerError);
    }

    return Err(NodeFindError::TypeMismatch);
}

/* browse part */
/// generate various links for a node in <li><a /></li>
fn make_simple_browse_node_link(
    node: &WzNodeArc,
    force_parse: bool,
    name: Option<&str>,
) -> Result<String, NodeFindError> {
    let mut url = node.get_path_from_root();

    if let Some(uol_string) = node.try_as_uol() {
        let mut uol_target_path = uol_string
            .get_string()
            .map_err(|_| NodeFindError::ServerError)?;
        uol_target_path.insert_str(0, "../");
        let uol_target = node.at_path_relative(&uol_target_path);
        if let Some(uol_target) = uol_target {
            url = uol_target.get_path_from_root();
        }
    }

    if !url.is_empty() {
        url.insert_str(0, "/");
    }
    if force_parse {
        url.push_str("?force_parse=true");
    }

    let name = name.unwrap_or(&node.name);

    let mut extra_link = String::new();

    if node.try_as_png().is_some() {
        extra_link.push_str(&format!(
            "<a href=\"/get_image{}\" target=\"_blank\">(image)</a>",
            url
        ));
    } else if node.try_as_sound().is_some() {
        extra_link.push_str(&format!(
            "<a href=\"/get_sound{}\" target=\"_blank\">(sound)</a>",
            url
        ));
    } else if node.try_as_uol().is_some() {
        extra_link.push_str("(uol link)");
    }

    Ok(format!(
        "<li><a href=\"/browse{}\">{}</a>{}</li>",
        url, name, extra_link
    ))
}

/// generate a list of childrens of a node in <ul></ul>
fn make_node_children_ul(
    node: &WzNodeArc,
    sort: bool,
    force_parse: bool,
) -> Result<String, NodeFindError> {
    let mut name_and_urls: Vec<(WzNodeName, String)> = vec![];

    for item in node.children.read().unwrap().values() {
        let name = item.name.clone();
        let html = make_simple_browse_node_link(item, force_parse, None)?;
        name_and_urls.push((name, html));
    }

    if sort {
        name_and_urls.sort_by(|(aname, _), (bname, _)| aname.cmp(bname.as_str()));
    }

    let mut result_string = String::new();

    if let Some(parent) = node.parent.upgrade() {
        result_string.push_str(&make_simple_browse_node_link(
            &parent,
            force_parse,
            Some(".."),
        )?);
    }

    for (_, html) in name_and_urls {
        result_string.push_str(&html);
    }

    Ok(format!("<ul>{}</ul>", result_string))
}

async fn simple_browse_root(
    State(ServerState { wz_root }): State<ServerState>,
    Query(param): Query<GetJsonParam>,
) -> Result<impl IntoResponse, NodeFindError> {
    let force_parse = param.force_parse.unwrap_or(true);
    let need_sort = param.sort.unwrap_or(true);

    let target = get_node_from_root(wz_root, "", force_parse)?;

    let result_ul = make_node_children_ul(&target, need_sort, force_parse)?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html;charset=utf-8")],
        Body::from(result_ul),
    ))
}

async fn simple_browse(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Result<impl IntoResponse, NodeFindError> {
    println!("access node use simple_browse: {}", path);
    let force_parse = param.force_parse.unwrap_or(true);
    let need_sort = param.sort.unwrap_or(true);

    let target = get_node_from_root(wz_root, &path, force_parse)?;

    let mut result_string = String::new();

    result_string.push_str(&format!(
        "<h2>Node Info:</h2><pre>{}</pre>",
        serde_json::to_string(&target.object_type).map_err(|_| NodeFindError::ServerError)?
    ));

    if target.children.read().unwrap().is_empty() {
        let value = target.to_simple_json().unwrap().to_string();

        if target.name.as_str() == "_inlink" {
            let value = target.try_as_string().unwrap().get_string().unwrap();
            match node_util::resolve_inlink(&value, &target) {
                Some(v) => {
                    let link_dest = v.get_full_path();
                    result_string.push_str(&make_simple_browse_node_link(
                        &v,
                        force_parse,
                        Some(&link_dest),
                    )?);
                }
                None => {
                    result_string
                        .push_str(&format!("can't not resolve _inlink <pre>{}</pre>", &value));
                }
            }
        } else if target.name.as_str() == "_outlink" {
            let value = target.try_as_string().unwrap().get_string().unwrap();
            match node_util::resolve_outlink(&value, &target, true) {
                Some(v) => {
                    let link_dest = v.get_full_path();
                    result_string.push_str(&make_simple_browse_node_link(
                        &v,
                        force_parse,
                        Some(&link_dest),
                    )?);
                }
                None => {
                    result_string
                        .push_str(&format!("can't not resolve _outlink <pre>{}</pre>", &value));
                }
            }
        } else if target.name.ends_with(".json") {
            let content = target.try_as_string().unwrap().get_string().unwrap();
            return Ok((
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json;charset=utf-8")],
                Body::from(content),
            ));
        } else if target.name.ends_with(".atlas") {
            let content = target.try_as_string().unwrap().get_string().unwrap();
            return Ok((
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/plain;charset=utf-8")],
                Body::from(content),
            ));
        } else if let Some(lua) = target.try_as_lua() {
            let content = lua.extract_lua().map_err(|_| NodeFindError::ServerError)?;
            return Ok((
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/plain;charset=utf-8")],
                Body::from(content),
            ));
        } else {
            result_string.push_str(&format!("<pre>{}</pre>", &value));
        }
    } else {
        result_string.push_str(&make_node_children_ul(&target, need_sort, force_parse)?);
    }

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html;charset=utf-8")],
        Body::from(result_string),
    ))
}
