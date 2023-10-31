use crate::RecvStream;
use crate::{plugins::calc_file_size, ProtResult};
use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};
use std::{collections::HashMap, io};
use std::path::{Path};
use tokio::fs::File;
use webparse::{BinaryMut, Buf, HeaderName, Request, Response};

lazy_static! {
    static ref DEFAULT_MIMETYPE: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::<&'static str, &'static str>::new();
        m.insert("doc", "application/msword");
        m.insert("pdf", "application/pdf");
        m.insert("rtf", "application/rtf");
        m.insert("xls", "application/vnd.ms-excel");
        m.insert("ppt", "application/vnd.ms-powerpoint");
        m.insert("rar", "application/application/x-rar-compressed");
        m.insert("swf", "application/x-shockwave-flash");
        m.insert("zip", "application/zip");
        m.insert("json", "application/json");
        m.insert("yaml", "text/plain");
        m.insert("mid", "audio/midi");
        m.insert("midi", "audio/midi");
        m.insert("kar", "audio/midi");
        m.insert("mp3", "audio/mpeg");
        m.insert("ogg", "audio/ogg");
        m.insert("m4a", "audio/m4a");
        m.insert("ra", "audio/x-realaudio");
        m.insert("gif", "image/gif");
        m.insert("jpeg", "image/jpeg");
        m.insert("jpg", "image/jpeg");
        m.insert("png", "image/png");
        m.insert("tif", "image/tiff");
        m.insert("tiff", "image/tiff");
        m.insert("wbmp", "image/vnd.wap.wbmp");
        m.insert("ico", "image/x-icon");
        m.insert("jng", "image/x-jng");
        m.insert("bmp", "image/x-ms-bmp");
        m.insert("svg", "image/svg+xml");
        m.insert("svgz", "image/svg+xml");
        m.insert("webp", "image/webp");
        m.insert("svg", "image/svg+xml");
        m.insert("css", "text/css");
        m.insert("html", "text/html");
        m.insert("htm", "text/html");
        m.insert("shtml", "text/html");
        m.insert("txt", "text/plain");
        m.insert("md", "text/plain");
        m.insert("xml", "text/xml");
        m.insert("3gpp", "video/3gpp");
        m.insert("3gp", "video/3gpp");
        m.insert("mp4", "video/mp4");
        m.insert("mpeg", "video/mpeg");
        m.insert("mpg", "video/mpeg");
        m.insert("mov", "video/quicktime");
        m.insert("webm", "video/webm");
        m.insert("flv", "video/x-flv");
        m.insert("m4v", "video/x-m4v");
        m.insert("wmv", "video/x-ms-wmv");
        m.insert("avi", "video/x-msvideo");
        m
    };

    static ref CURRENT_DIR: String = {
        if let Ok(path) = std::env::current_dir() {
            path.to_string_lossy().to_string()
         } else {
             String::new()
         }
    };
}


fn default_root() -> String {
    CURRENT_DIR.to_string()
}

fn default_mimetype() -> String {
    "application/octet-stream".to_string()
}

fn default_bool_true() -> bool {
    true
}

fn default_status() -> u16 {
    404
}

fn default_hide() -> Vec<String> {
    vec![]
}

fn default_index() -> Vec<String> {
    vec!["index.html".to_string(), "index.htm".to_string()]
}

fn default_precompressed() -> Vec<String> {
    vec!["gzip".to_string(), "br".to_string()]
}

/// 代理类, 一个代理类启动一种类型的代理
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileServer {
    // #[serde(default = "default_root")]
    pub root: Option<String>,
    #[serde(default)]
    pub prefix: String,
    #[serde(default="default_mimetype")]
    pub default_mimetype: String,
    #[serde(default="default_hide")]
    pub hide: Vec<String>,
    #[serde(default = "default_index")]
    pub index: Vec<String>,
    #[serde(default = "default_status")]
    pub status: u16,
    #[serde(default = "default_precompressed")]
    pub precompressed: Vec<String>,
    #[serde(default)]
    pub disable_compress: bool,
    #[serde(default)]
    pub browse: bool,
}

const HEAD_HTML_PRE: &'static str = r#"
<html><head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width">
    <title>Index of 
"#;
const HEAD_HTML_AFTER: &'static str = r#"
    </title>
    <style type="text/css">i.icon { display: block; height: 16px; width: 16px; }
table tr { white-space: nowrap; }
td.perms {}
td.file-size { text-align: right; padding-left: 1em; }
td.display-name { padding-left: 1em; }
i.icon-_blank {
  background-image: url("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGXRFWHRTb2Z0d2FyZQBBZG9iZSBJbWFnZVJlYWR5ccllPAAAAWBJREFUeNqEUj1LxEAQnd1MVA4lyIEWx6UIKEGUExGsbC3tLfwJ/hT/g7VlCnubqxXBwg/Q4hQP/LhKL5nZuBsvuGfW5MGyuzM7jzdvVuR5DgYnZ+f99ai7Vt5t9K9unu4HLweI3qWYxI6PDosdy0fhcntxO44CcOBzPA7mfEyuHwf7ntQk4jcnywOxIlfxOCNYaLVgb6cXbkTdhJXq2SIlNMC0xIqhHczDbi8OVzpLSUa0WebRfmigLHqj1EcPZnwf7gbDIrYVRyEinurj6jTBHyI7pqVrFQqEbt6TEmZ9v1NRAJNC1xTYxIQh/MmRUlmFQE3qWOW1nqB2TWk1/3tgJV0waVvkFIEeZbHq4ElyKzAmEXOx6gnEVJuWBzmkRJBRPYGZBDsVaOlpSgVJE2yVaAe/0kx/3azBRO0VsbMFZE3CDSZKweZfYIVg+DZ6v7h9GDVOwZPw/PoxKu/fAgwALbDAXf7DdQkAAAAASUVORK5CYII=");
}

i.icon-_page {
  background-image: url("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGXRFWHRTb2Z0d2FyZQBBZG9iZSBJbWFnZVJlYWR5ccllPAAAAmhJREFUeNpsUztv01AYPfdhOy/XTZ80VV1VoCqlA2zQqUgwMEErWBALv4GJDfEDmOEHsFTqVCTExAiiSI2QEKJKESVFFBWo04TESRzfy2c7LY/kLtf2d8+555zvM9NaI1ora5svby9OnbUEBxgDlIKiWjXQeLy19/X17sEtcPY2rtHS96/Hu0RvXXLz+cUzM87zShsI29DpHCYt4E6Box4IZzTnbDx7V74GjhOSfwgE0H2638K9h08A3iHGVbjTw7g6YmAyw/BgecHNGGJjvfQhIfmfIFDAXJpjuugi7djIFVI4P0plctgJQ0xnFe5eOO02OwEp2VkhSCnC8WOCdqgwnzFx4/IyppwRVN+XYXsecqZA1pB48ekAnw9/4GZx3L04N/GoTwEjX4cNH5vlPfjtAIYp8cWrQutxrC5Mod3VsXVTMFSqtaE+gl9dhaUxE2tXZiF7nYiiatJ3v5s8R/1yOCNLOuwjkELiTbmC9dJHpIaGASsDkoFQGJQwHWMcHWJYOmUj1OjvQotuytt5nHMLEGkCyx6QU384jwkUAd2sxJbS/QShZtg/8rHzzQOzSaFhxQrA6YgQMQHojCUlgnCAAvKFBoXXaHfArSCZDE0gyWJgFIKmvUFKO4MUNIk2a4+hODtDUVuJ/J732AKS6ZtImdTyAQQB3bZN8l9t75IFh0JMUdVKsohsUPqRgnka0tYgggYpCHkKGTsHI5NOMojB4iTICCepvX53AIEfQta1iUCmoTiBmdEri2RgddKFhuJoqb/af/yw/d3zTNM6UkaOfis62aUgddAbnz+rXuPY+Vnzjt9/CzAAbmLjCrfBiRgAAAAASUVORK5CYII=");
}

i.icon-zip {
  background-image: url("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGXRFWHRTb2Z0d2FyZQBBZG9iZSBJbWFnZVJlYWR5ccllPAAAAm9JREFUeNpsk0tv00AUhc+MY6dOmgeFJg1FoVVpUWlFC0s2IFF1jxBbhKj4BSxYdscPYcEmQmIDq0gsERIViy4TpD7VFzF1Ho5je2a4thOqNhlp5Mz4zudzzp0wpRTC8fPrk0/TC6+fDtYicLH97T1Kc2vQDcs+rH3eUAxVznn0fn1DRM8E+iOdv5ct3XmZG6yVlNj6solUbgVTt0q5FGtX6vXqC6VklTE+KAO/OODHSIQPRQpsXC+kkEz2ELA0ystv84tLzyucsbWByisAGf+QAS2CCDRRLMJMmxC+i8C4jdLCm/zM7OOKFGptcO6/BTpJ0yeQB0Y+mfKQuZZG0jQgeRbW8Xdomobs9LN8scc+UPHNy4Dwq8IljotIIQEm59/RoSyM1CKkXKZNBm7kIVgyM6wgAnSgRK9vqQfHPiMFDHqyFVsLR9Cm0o4YzoAASrSjCelQfRPb1Vc4qn0EY5L2W9GEaBLcxQgFHpGbkMIDJ69e+wjJ8VXqRgKid0r7ftQdxkRs9SqA2kgAm14SSIQh9uhuLGPMnKJs/5KquL1x0N0RCsizigoDaLqBdHoMiyvrlBsHVx1wphD4BCewoqxGKKDwAgtOy8JufYuk+5golGGaGZwc1sIGoDz3AOPZSVLaHgVwydoJDM1H4DbQODughB3YpOD44HfoHgnu4e7So0uAi0stHLJ3Aud8B9bpHu6vPoSu9TtDl6tUuoFiIYOgu0+158MKmOxomtyD3Qi/3MTR7i8K0EDG1GHO5DE3X4DvNahZlJOwEkOATvdPc2//hx3mXJ5lFJaF8K8bStd0YGfnOJbMGex21x6c+yfAAOlIPDJzr7cLAAAAAElFTkSuQmCC");
}

</style>
  </head>
"#;

impl FileServer {
    pub fn new(root: String, prefix: String) -> Self {
        
        let mut config = Self {
            root: if root.len() > 0 { Some(root) } else { None },
            prefix,
            hide: vec![],
            default_mimetype: default_mimetype(),
            index: default_index(),
            status: 404,
            precompressed: vec![],
            disable_compress: false,
            browse: true,
        };
        config.fix_default();
        config
    }

    pub fn fix_default(&mut self) {
        if self.prefix.ends_with("/") {
            self.prefix = self.prefix.strip_suffix("/").unwrap().to_string();
        }
    }

    pub fn set_prefix(&mut self, prefix: String) {
        self.prefix = prefix;
        self.fix_default();
    }

    pub fn set_browse(&mut self, browse: bool) {
        self.browse = browse;
    }

    pub fn set_disable_compress(&mut self, disable: bool) {
        self.disable_compress = disable;
    }

    pub fn is_hide_path(&self, path: &Path) -> bool {
        let value = path.to_string_lossy();
        for hide in &self.hide {
            if value.contains(&*hide) {
                return true
            }
        }
        false
    }

    fn ret_error_msg(&self, msg: &'static str) -> Response<RecvStream> {
        Response::builder()
                .status(self.status)
                .body(msg)
                .unwrap()
                .into_type()
    }

    pub async fn deal_request(
        &self,
        req: Request<RecvStream>,
    ) -> ProtResult<Response<RecvStream>> {
        let path = req.path().clone();
        // 无效前缀，无法处理
        if !path.starts_with(&self.prefix) {
            return Ok(self.ret_error_msg("unknow path"));
        }
        let root = self.root.clone().unwrap_or(CURRENT_DIR.clone());
        let root_path = Path::new(&root);
        let href = "/".to_string() + path.strip_prefix(&self.prefix).unwrap();
        let real_path = root.clone() + &href;
        let mut real_path = Path::new(&real_path).to_owned();
        // 必须保证不会跑出root设置的目录之外，如故意访问`../`之类的
        if !real_path.starts_with(root_path) || self.is_hide_path(root_path.as_ref()) {
            return Ok(self.ret_error_msg("can't view parent file"));
        }
        
        // 访问路径是目录，尝试是否有index的文件，如果有还是以文件访问
        if real_path.is_dir() {
            for index in &self.index {
                let new_path = real_path.join(index);
                if new_path.exists() {
                    real_path = new_path;
                    break;
                }
            }
        }

        // 访问为目录，如果启用目录访问，则返回当前的文件夹的内容
        if real_path.is_dir() {
            if !self.browse {
                return Ok(self.ret_error_msg("can't view parent file"));
            }
            let mut binary = BinaryMut::new();
            binary.put_slice(HEAD_HTML_PRE.as_bytes());
            binary.put_slice(href.as_bytes());
            binary.put_slice(HEAD_HTML_AFTER.as_bytes());
            binary.put_slice(format!("<body><h1>Index Of {}</h1>", href).as_bytes());
            binary.put_slice("<table>\r\n<tbody>".as_bytes());

            let mut folder_binary = BinaryMut::new();
            let mut file_binary = BinaryMut::new();
            for entry in real_path.read_dir()? {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if self.is_hide_path(path.as_ref()) {
                        continue;
                    }
                    let new = path.strip_prefix(root_path).unwrap();
                    let value = "/".to_string() + new.to_str().unwrap();
                    let value = value.replace("\\", "/");
                    let op_ref = if path.is_dir() {
                        &mut folder_binary
                    } else {
                        &mut file_binary
                    };
                    op_ref.put_slice("<tr>".as_bytes());
                    let filename = path.file_name().unwrap().to_str().unwrap();
                    if path.is_dir() {
                        op_ref.put_slice("<td><i class=\"icon icon-_blank\"></i></td>".as_bytes());
                        op_ref.put_slice("<td class=\"file-size\"><code></code></td>".as_bytes());
                        op_ref.put_slice(
                            format!("<td><a href=\"{}{}\">{}</td>", self.prefix, value, filename)
                                .as_bytes(),
                        );
                    } else {
                        op_ref.put_slice("<td><i class=\"icon icon-_page\"></i></td>".as_bytes());
                        if let Ok(meta) = path.metadata() {
                            op_ref.put_slice(
                                format!(
                                    "<td class=\"file-size\"><code>{}</code></td>",
                                    calc_file_size(meta.len())
                                )
                                .as_bytes(),
                            );
                        } else {
                            op_ref
                                .put_slice("<td class=\"file-size\"><code></code></td>".as_bytes());
                        }
                        op_ref.put_slice(
                            format!("<td><a href=\"{}{}\">{}</td>", self.prefix, value, filename)
                                .as_bytes(),
                        );
                    }
                    op_ref.put_slice("</tr>".as_bytes());
                    println!("{:?}", entry.path());
                }
            }
            binary.put_slice(folder_binary.chunk());
            binary.put_slice(file_binary.chunk());
            binary.put_slice("</tbody>\r\n</table>".as_bytes());
            binary.put_slice("<br><address>wengmeng <a href=\"https://github.com/tickbh/wenmeng\">wenmeng</a></address>".as_bytes());
            binary.put_slice("</body></html>".as_bytes());

            let recv = RecvStream::only(binary.freeze());
            let builder = Response::builder().version(req.version().clone());
            let mut response = builder
                .header(HeaderName::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(recv)
                .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
            if self.disable_compress {
                response.headers_mut().insert(HeaderName::CONTENT_ENCODING, "");
            }
            return Ok(response);
        } else {
            // 访问为文件，判断当前的后缀，返回合适的mimetype，如果有合适的预压缩文件，也及时返回
            if self.is_hide_path(path.as_ref()) {
                return Ok(self.ret_error_msg("can't view file"));
            }
            // 获取后缀
            let extension = if let Some(s) = real_path.extension() {
                s.to_string_lossy().to_string()
            } else {
                String::new()
            };

            let application = if let Some(s) = DEFAULT_MIMETYPE.get(&*extension) {
                s.to_string()
            } else {
                self.default_mimetype.to_string()
            };
            //查找是否有合适的预压缩文件
            if let Some(accept) = req.headers().get_option_value(&HeaderName::ACCEPT_ENCODING) {
                for pre in &self.precompressed {
                    // 得客户端发送支持该格式
                    if !accept.contains(pre.as_bytes()) {
                        continue;
                    }
                    let mut new = real_path.clone();
                    new.as_mut_os_string().push(".");
                    match &**pre {
                        "gzip" => new.as_mut_os_string().push("gz"),
                        "br" => new.as_mut_os_string().push("br"),
                        _ => continue,
                    };
                    // 如果预压缩文件存在
                    if new.exists() {
                        println!("convert to new file {}", new.to_string_lossy());
                        let file = File::open(new).await?;
                        let data_size = file.metadata().await?.len();
                        let mut recv = RecvStream::new_file(file, data_size);
                        match &**pre {
                            "gzip" => recv.set_compress_origin_gzip(),
                            "br" => recv.set_compress_brotli(),
                            _ => unreachable!(),
                        }
                        let builder = Response::builder().version(req.version().clone());
                        let mut response = builder
                            .header(HeaderName::CONTENT_ENCODING, pre.to_string())
                            .header(
                                HeaderName::CONTENT_TYPE,
                                format!("{}; charset=utf-8", application),
                            )
                            .header(HeaderName::TRANSFER_ENCODING, "chunked")
                            .body(recv)
                            .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
                        if self.disable_compress {
                            response.headers_mut().insert(HeaderName::CONTENT_ENCODING, "");
                        }
                        return Ok(response);
                    }
                }
            }

            if !real_path.exists() {
                return Ok(self.ret_error_msg("can't view file"));
            }

            let file = File::open(real_path).await?;
            let data_size = file.metadata().await?.len();
            let recv = RecvStream::new_file(file, data_size);
            let builder = Response::builder().version(req.version().clone());
            let mut response = builder
                .header(
                    HeaderName::CONTENT_TYPE,
                    format!("{}; charset=utf-8", application),
                )
                .header(HeaderName::TRANSFER_ENCODING, "chunked")
                .body(recv)
                .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
            if self.disable_compress {
                response.headers_mut().insert(HeaderName::CONTENT_ENCODING, "");
            }
            return Ok(response);
        }
    }
}

