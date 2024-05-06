use axum::{
    extract::{rejection::JsonRejection, FromRequest, MatchedPath, Request, State, Path, Query}, 
    http::{version, header,StatusCode}, 
    response::{Html, IntoResponse, Response}, 
    routing::{get, post}, Json, Router, body::Body
};
use std::io::{BufWriter, Cursor};
use std::sync::{Arc, RwLock, Mutex};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wz_reader::{WzNodeArc, WzNodeCast, node, util::{resolve_base, walk_node}, version::WzMapleVersion, property};
use image::ImageFormat;

#[derive(Clone)]
pub struct ServerState {
    pub wz_root: Arc<RwLock<Option<WzNodeArc>>>,
}

// run example with `cargo run --package wz_reader --example with_axum --features json`
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
        </ul>"
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
            InitWzError::MissingParam => {
                Response::builder().status(StatusCode::BAD_REQUEST).body("should passing path and version".into()).unwrap()
            }
            InitWzError::ParseError => {
                Response::builder().status(StatusCode::BAD_REQUEST).body("wz parse error".into()).unwrap()
            }
            InitWzError::VersionError => {
                Response::builder().status(StatusCode::BAD_REQUEST).body("passing wrong wz version".into()).unwrap()
            }
            InitWzError::IoError => {
                Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body("file error".into()).unwrap()
            }
        }
    }
}
async fn init_wz_root(
    State(ServerState { wz_root }): State<ServerState>,
    Json(body): Json<Value>,
) -> Response {
    let base_path = body.get("path").and_then(|v| v.as_str());
    let version = body.get("version").and_then(|v| v.as_str());

    if base_path.is_none() || version.is_none() {
        return InitWzError::MissingParam.into_response();
    }

    let version = match version.unwrap() {
        "BMS" => Some(WzMapleVersion::BMS),
        "GMS" => Some(WzMapleVersion::GMS),
        "EMS" => Some(WzMapleVersion::EMS),
        _ => None,
    };

    let base_path = base_path.unwrap();

    if base_path.is_empty() {
        return InitWzError::MissingParam.into_response();
    }

    let result = resolve_base(base_path, version);
    if result.is_err() {
        return InitWzError::IoError.into_response();
    }
    let base_node = result.unwrap();
    let mut wz_root = wz_root.write().unwrap();
    *wz_root = Some(base_node);

    return StatusCode::OK.into_response();
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
            NodeFindError::Uninitialized => {
                (StatusCode::BAD_REQUEST, "wz uninitialized, please do `/init_wz_root` first").into_response()
            }
            NodeFindError::NotFound => {
                (StatusCode::NOT_FOUND, "node not found").into_response()
            }
            NodeFindError::TypeMismatch => {
                (StatusCode::BAD_REQUEST, "node type can't use on this route").into_response()
            },
            NodeFindError::ServerError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "something wrong when parsing data").into_response()
            },
            NodeFindError::ParseError => {
                (StatusCode::BAD_REQUEST, "node parse error").into_response()
            }
        }
    }
}
impl From<node::Error> for NodeFindError {
    fn from(e: node::Error) -> Self {
        NodeFindError::ParseError
    }
}

fn get_node_from_root(root: Arc<RwLock<Option<WzNodeArc>>>, path: &str, force_parse: bool) -> Result<WzNodeArc, NodeFindError> {
    let wz_root = root.read().unwrap();

    if wz_root.is_none() {
        return Err(NodeFindError::Uninitialized);
    }

    let wz_root = wz_root.as_ref().unwrap();
    let wz_root = wz_root.read().unwrap();

    let target = if force_parse {
        wz_root.at_path_parsed(&path)
    } else {
        wz_root.at_path(&path).ok_or(node::Error::NodeNotFound)
    };

    let target = target?;

    if force_parse {
        node::parse_node(&target)?;
    }

    Ok(target)
}

/* grabe json part */
#[derive(Deserialize)]
struct GetJsonParam {
    simple: Option<bool>,
    force_parse: Option<bool>,
}

async fn get_json(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Response {
    println!("try to get path's json: {}", path);
    let is_simple = param.simple.unwrap_or(false);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = match get_node_from_root(wz_root, &path, force_parse) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    let json = if is_simple {
        target.read().unwrap().to_simple_json()
    } else {
        target.read().unwrap().to_json()
    };

    let json = json.unwrap();

    (StatusCode::OK, [(header::CONTENT_TYPE, "application/json;charset=utf-8")], Body::from(json.to_string())).into_response()
}

/* grabe image part */
async fn get_image(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Response {
    println!("try to get image: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = match get_node_from_root(wz_root, &path, force_parse) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    let target_read = target.read().unwrap();
    
    if let Some(_) = target_read.try_as_png() {
        let img = property::get_image(&target);

        if img.is_err() {
            return NodeFindError::ServerError.into_response();
        }

        let img = img.unwrap();

        let mut buf = BufWriter::new(Cursor::new(Vec::new()));
        img.write_to(&mut buf, ImageFormat::Bmp).expect("write image error");

        let body = Body::from(buf.into_inner().unwrap().into_inner());

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/bmp")
            .header(header::CACHE_CONTROL, "max-age=3600")
            .body(body).unwrap()
            .into_response();
    } 

    return NodeFindError::TypeMismatch.into_response();
}

/* grabe image urls part */
async fn get_image_urls(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Response {
    println!("try to get images from a node: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = match get_node_from_root(wz_root, &path, force_parse) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    let urls = Mutex::new(Vec::new());
    
    walk_node(&target, force_parse, &|node| {
        let node_read = node.read().unwrap();
        if let Some(_) = node_read.try_as_png() {
            let path = node_read.get_full_path().replace("Base/", "");
            let mut urls = urls.lock().unwrap();
            urls.push(path);
        }
    });

    let urls = urls.into_inner().unwrap();
    
    let json = serde_json::to_string(&urls).unwrap();

    (StatusCode::OK, [(header::CONTENT_TYPE, "application/json;charset=utf-8")], Body::from(json)).into_response()
}

/* grabe sound part */
async fn get_sound(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Response {
    println!("try to get sound: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let target = match get_node_from_root(wz_root, &path, force_parse) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    let target_read = target.read().unwrap();
    
    if let Some(sound) = target_read.try_as_sound() {
        let sound_buf = sound.get_buffer();

        let mini = match sound.sound_type {
            property::WzSoundType::Wav => "audio/wav",
            _ => "audio/mpeg",
        };

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mini)
            .body::<Body>(sound_buf.into()).unwrap()
            .into_response();
    } 

    return NodeFindError::TypeMismatch.into_response();
}