use std::{env, fs, io, path};
use std::io::{BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Result};
use xml::reader::{EventReader, XmlEvent};
use reqwest;


#[derive(Default, Debug, Clone)]
struct UserInfo {
    id: String,
    name: String,
    path: PathBuf,
}

fn main() -> Result<()> {
    println!("开始查找帐号...");
    let mut infos = get_account_info()?;
    println!("查找完成，共找到 {} 个帐号:", infos.len());
    if infos.len() == 0 {
        return Err(anyhow!("未找到帐号"));
    } else if infos.len() > 1 {
        println!("0:全部提取");
        for (index, info) in infos.iter().enumerate() {
            println!("{}:id({}), name({})", index + 1, info.id, info.name);
        }
        println!("请输入要提取的帐号序号:");
        let mut input = String::new(); // 创建一个新的字符串用来存放输入的内容
        infos = match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let mut infos = infos;
                let index = input.trim().parse::<usize>()?;
                if index > 0 && index <= infos.len() {
                    let info = infos[index - 1].clone();
                    infos = vec![info]
                } else if index != 0 {
                    println!("序号错误");
                    return Err(anyhow!("序号错误"));
                }
                Ok(infos)
            }
            Err(err) => {
                Err(err)
            }
        }?;
    }
    for info in infos.iter() {
        println!("开始备份fav.archive文件: id({}), name({})", info.id, info.name);
        let path = backup_fav(info)?;
        let urls = get_stickers(path)?;
        let path = path::Path::new(&info.name);
        match fs::metadata(path).map(|m| m.is_dir()) {
            Ok(_) => {}
            Err(_) => {
                fs::create_dir(info.name.clone())?;
            }
        }
        for (file_name, target) in urls {
            download(target, path.join(file_name + ".gif"))?;
        }
    }
    Ok(())
}

fn download(target_url: String, path: PathBuf) -> Result<()> {
    let mut resp = reqwest::blocking::get(target_url)?;
    let mut file = fs::File::create(path)?;
    io::copy(&mut resp, &mut file)?;
    Ok(())
}

fn get_stickers(path: PathBuf) -> Result<Vec<(String, String)>> {
    let mut ret = Vec::new();
    let file = fs::File::open(path)?;
    let file = BufReader::new(file);
    let parser = EventReader::new(file);

    let mut path = Vec::new();
    let mut v: (String, String) = (String::new(), String::new());

    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, .. }) => {
                path.push(name.local_name);
            }
            Ok(XmlEvent::EndElement { .. }) => {
                path.pop();
            }
            Ok(XmlEvent::Characters(s)) => {
                if path == ["plist", "dict", "array", "string"] {
                    if !s.starts_with("http") {
                        v.0 = s.clone();
                    } else {
                        v.1 = s.clone();
                        ret.push(v.clone());
                    }
                }
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }
    Ok(ret)
}

fn backup_fav(info: &UserInfo) -> Result<PathBuf> {
    let path = info.path.clone();
    let path = path.join("Stickers/fav.archive");
    if !path.exists() {
        return Err(anyhow!("fav.archive 文件不存在"));
    }
    let mut temp_path = env::temp_dir().join("fav.archive");
    println!("temp_path: {:?}", temp_path);
    fs::copy(&path, &mut temp_path)?;
    println!("开始提取表情包: {:?}", path);
    Command::new("plutil")
        .arg("-convert")
        .arg("xml1")
        .arg(&temp_path)
        .output()?;
    Ok(temp_path)
}

fn get_account_info() -> Result<Vec<UserInfo>> {
    let home_dir = dirs::home_dir().ok_or(anyhow!("Home directory not found"))?;
    let base_path = home_dir.join("Library/Containers/com.tencent.xinWeChat/Data/Library/Application Support/com.tencent.xinWeChat/2.0b4.0.9/"); // 指定路径
    let paths = visit_dirs(base_path.as_path());
    let mut infos = Vec::new();
    for path in paths {
        let userinfo_path = path.join("account/userinfo.data");
        let data = fs::read(userinfo_path)?;
        let search_bytes = hex::decode("9201")?;
        let mut info: UserInfo = Default::default();
        info.id = search(&data, &search_bytes)?;
        let search_bytes = hex::decode("9a0100a201")?;
        info.name = search(&data, &search_bytes)?;
        info.path = path;
        infos.push(info);
    }
    Ok(infos)
}

fn search(data: &[u8], search_bytes: &[u8]) -> Result<String> {
    if let Some(position) = data.windows(search_bytes.len()).position(|window| window == search_bytes) {
        // 检查是否存在足够的空间来读取长度和字符串
        if position + search_bytes.len() < data.len() {
            // 获取字符串长度
            let str_len = data[position + search_bytes.len()] as usize;

            // 检查是否存在足够的空间来读取整个字符串
            if position + search_bytes.len() + 1 + str_len <= data.len() {
                // 读取字符串
                let start = position + search_bytes.len() + 1;
                let end = start + str_len;
                let string_data = &data[start..end];

                // 将字节转换为字符串
                let s = String::from_utf8(string_data.to_vec())?;
                return Ok(s);
            } else {
                Err(anyhow!("String length exceeds file size."))?;
            }
        } else {
            Err(anyhow!("No space in file for string length and string."))?;
        }
    }
    Err(anyhow!("Sequence not found"))?
}

fn visit_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut ret = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    // 检查是否存在名为 'account' 的子目录
                    let account_dir = path.join("account");
                    if account_dir.exists() && account_dir.is_dir() {
                        let userinfo_dir = account_dir.join("userinfo.data");
                        if userinfo_dir.exists() && userinfo_dir.is_file() {
                            ret.push(path.clone())
                        }
                    }
                }
            }
        }
    }
    ret
}