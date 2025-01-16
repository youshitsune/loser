use tiny_http::{Server, Response, Request};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;

const INDEX: &str = "index.json";

struct Tokenizer<'a> {
    content: &'a [char],
}

impl<'a> Tokenizer<'a> {
    fn new(content: &'a [char]) -> Self{
        Self {content}
    }

    fn next_token(&mut self) -> Option<&'a [char]>{
        while self.content.len() > 0 && self.content[0].is_whitespace() {
            self.content = &self.content[1..];
        }

        if self.content.len() == 0 {
            return None
        }

        if self.content[0].is_alphabetic() {
            let mut i = 0;
            while i < self.content.len() && self.content[i].is_alphabetic(){
                i+=1;
            }
            let token = &self.content[0..i];
            self.content = &self.content[i..];
            return Some(token)
        }

        if self.content[0].is_numeric() {
            let mut i = 0;
            while i < self.content.len() && self.content[i].is_numeric() {
                i+=1;
            }
            let token = &self.content[0..i];
            self.content = &self.content[i..];
            return Some(token)
        }

        let token = &self.content[0..1];
        self.content = &self.content[1..];
        Some(token)
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = &'a [char];
    fn next(&mut self) -> Option<&'a [char]>{
        self.next_token()
    }
}

fn read_text_file(path: &PathBuf) -> Vec<char>{
    return fs::read_to_string(path).unwrap().chars().map(|x| {x.to_ascii_uppercase()}).collect::<Vec<char>>()
}

fn read_pdf_file(path: &PathBuf) -> Vec<char>{
    let bytes = fs::read(path).unwrap();
    let t = pdf_extract::extract_text_from_mem(&bytes).unwrap().chars().map(|x| {x.to_ascii_uppercase()}).collect::<Vec<char>>();
    return t
}

fn tf(term: &String, col: &HashMap<String, usize>) -> f32{
    if col.contains_key(term) {
        return (col[term] as f32)/(col.len() as f32);
    } else {
        return 0.0
    }
}

fn idf(term: &String, index_table: &HashMap<PathBuf, HashMap<String, usize>>) -> f32{
    let mut n = 0;
    for v in index_table.values(){
        if v.get(term) != None {
            n+=1;
        }
    }
    return (((index_table.len()+1) as f32)/(n as f32)).log10();
}


fn root(req: Request) {
    let response = Response::from_file(File::open("/home/youshitsune/Projects/loser/src/index.html").unwrap());
    req.respond(response).expect("Can't respond");
}

fn search(query: &[char]) -> Option<Vec<String>>{
    if fs::exists(INDEX).unwrap() {
        let index_file = fs::File::open(INDEX);
        let index_table: HashMap<PathBuf, HashMap<String, usize>> = serde_json::from_reader(index_file.unwrap()).unwrap();

        let mut r = HashMap::<PathBuf, f32>::new();
        for token in Tokenizer::new(&query){
            let term = token.iter().collect::<String>();
            let idfv = idf(&term, &index_table);

            for (path, doc) in index_table.iter() {
                let t = tf(&term, &doc)*idfv;

                if let Some(count) = r.get_mut(&path.to_path_buf()){
                    *count += t;
                } else {
                    r.insert(path.to_path_buf(), t);
                }
            }
        }
        let mut stats: Vec<(PathBuf, f32)> = vec![];
        for (i, j) in r.iter() {
            stats.push((i.clone(), j.clone()));
        }
        stats.sort_by(|a, b| {(b.1).partial_cmp(&a.1).unwrap()});
        let mut results: Vec<String> = vec![];

        if stats.len() > 10 {
            for i in 0..10{
                if stats[i].1 > 0.0 {
                    results.push(stats[i].0.to_str().unwrap().to_string());
                }
            }
            return Some(results)
        } 

        for i in 0..stats.len(){
            if stats[i].1 > 0.0 {
                results.push(stats[i].0.to_str().unwrap().to_string());
            }
        }

        return Some(results)
    }

    return None
}
fn searchapi(mut req: Request) {
    let mut body = String::new();
    let _ = req.as_reader().read_to_string(&mut body).unwrap();
    let json_data = json::parse(&body).unwrap();
    let data = json_data["query"].to_string();
    let query = data.chars().map(|x| {x.to_ascii_uppercase()}).collect::<Vec<_>>();
    let r = search(&query);
    if r != None {
        let rd = json::stringify(&r.unwrap()[0..]);
        req.respond(Response::from_string(&rd)).expect("Can't respond");
    } else {
        req.respond(Response::from_string("You need to reindex files first")).expect("Can't respond");
    }
}

fn reindex(path: &str) -> bool {
    if fs::exists(path).unwrap() {
        let dir = fs::read_dir(path).unwrap();

        let mut tf = HashMap::<PathBuf, HashMap<String, usize>>::new();

        for file in dir {
            let file_path = file.unwrap().path();
            let file_name = file_path.to_str().unwrap();
            let mut map = HashMap::<String, usize>::new();

            if file_name.contains(".md") || file_name.contains(".txt"){
                let ctx = read_text_file(&file_path);
                for token in Tokenizer::new(&ctx){
                    let token = token.iter().collect::<String>();
                    if let Some(count) = map.get_mut(&token){
                        *count += 1;
                    } else {
                        map.insert(token, 1);
                    }
                }
            } 

            else if file_name.contains(".pdf") {
                let ctx = read_pdf_file(&file_path);
                for token in Tokenizer::new(&ctx) {
                    let token = token.iter().collect::<String>();
                    if let Some(count) = map.get_mut(&token){
                        *count += 1;
                    } else {
                        map.insert(token, 1);
                    }
                }
            }

            tf.insert(file_path, map);
        }
        let index_file = fs::File::create_new(INDEX).unwrap();
        serde_json::to_writer_pretty(index_file, &tf).unwrap();
        return true
    }
    return false
}

fn indexapi(mut req: Request) {
    let mut buf = String::new();
    let _ = req.as_reader().read_to_string(&mut buf);
    let data = json::parse(&buf).unwrap();
    println!("{data:?}");
    if reindex(&data["data"].to_string()) {
        req.respond(Response::from_string("Reindexing completed")).expect("Can't respond");
    } else {
        req.respond(Response::from_string("Path doesn't exist")).expect("Can't respond");
    }
}

fn main(){
    let server = Server::http("0.0.0.0:8080").unwrap();

    loop {
        let request = match server.recv() {
            Ok(rq) => rq,
            Err(e) => {eprintln!("ERROR: {}", e); break},
        };

        match request.url(){
            "/" => {root(request)},
            "/search" => {searchapi(request)},
            "/reindex" => {indexapi(request)},
            _ => {request.respond(Response::from_string("Path doesn't exist")).expect("Can't respond")},
        };
    }
}
