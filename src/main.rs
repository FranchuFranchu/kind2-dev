#![allow(dead_code)]
#![allow(unused_imports)]
#![feature(slice_split_once)]

mod book;
mod form;
mod info;
mod term;

use book::*;
use form::*;
use info::*;
use term::*;

use highlight_error::highlight_error;
use std::{env, fs::File, io::Write, process::Command};
use TSPL::Parser;

TSPL::new_parser!(KindParser);

fn generate_check_hvm1(book: &Book, command: &str, arg: &str) -> String {
  //let used_defs = book.defs.keys().collect::<Vec<_>>().iter().map(|name|
  // format!("(Pair \"{}\" Book.{})", name, name)).collect::<Vec<_>>().join(" ");
  let kind_hvm1 = include_str!("./kind2.hvm1");
  let book_hvm1 = book.to_hvm1();
  let main_hvm1 = match command {
    "check" => format!("Main = (API.check Book.{})\n", arg),
    //"check" => format!("Main = (API.check.many [{}])\n", used_defs),
    "run" => format!("Main = (API.normal Book.{})\n", arg),
    _ => panic!("invalid command"),
  };
  let hvm1_code = format!("{}\n{}\n{}", kind_hvm1, book_hvm1, main_hvm1);
  return hvm1_code;
}

fn generate_check_hvm2(book: &Book, command: &str, arg: &str) -> String {
  //let used_defs = book.defs.keys().collect::<Vec<_>>().iter().map(|name|
  // format!("(Pair \"{}\" Book.{})", name, name)).collect::<Vec<_>>().join(" ");
  let kind_hvm2 = include_str!("./kind2.hvm2");
  let book_hvm2 = book.to_hvm2();
  let main_hvm2 = match command {
    "check" => format!("Main = (API.check Book.{})\n", arg),
    "run" => format!("Main = (API.normal Book.{})\n", arg),
    _ => panic!("invalid command"),
  };
  let hvm2_code = format!("{}\n{}\n{}", kind_hvm2, book_hvm2, main_hvm2);
  return hvm2_code;
}
fn generate_check_hs(book: &Book, command: &str, arg: &str) -> String {
  let kind_hs = include_str!("./kind2.hs");
  let kind_hs = kind_hs.lines().filter(|line| !line.starts_with("xString")).collect::<Vec<_>>().join("\n");
  let book_hs = book.to_hs();
  let main_hs = match command {
    "check" => format!("main = (apiCheck {})\n", Term::to_hs_name(arg)),
    "run" => format!("Main = (apiNormal {})\n", Term::to_hs_name(arg)),
    _ => panic!("invalid command"),
  };
  let hs_code = format!("{}\n{}\n{}", kind_hs, book_hs, main_hs);
  return hs_code;
}

pub fn get_infos(book: &Book, cmd: &str, file: &str, runtime: &str) -> (Vec<Info>, String) {
  // Auto-formats the definition.
  let defn = book.defs.get(file).unwrap();
  let form = book.format_def(file, defn).flatten(60);

  // Overwrites the original file with the formatted version.
  File::create(&format!("./{}.kind2", file))
    .and_then(|mut file| file.write_all(form.as_bytes()))
    .unwrap_or_else(|_| panic!("Failed to auto-format '{}.kind2'.", file));

  let (stdout, stderr) = match runtime {
    "--hvm1" => {
      // Generates the HVM1 checker.
      let check_hvm1 = generate_check_hvm1(&book, cmd, file);
      let mut file = File::create(".check.hvm1").expect("Failed to create '.check.hvm1'.");
      file.write_all(check_hvm1.as_bytes()).expect("Failed to write '.check.hvm1'.");

      // Calls HVM1 and get outputs.
      let output = Command::new("hvm1")
        .arg("run")
        .arg("-t")
        .arg("1")
        .arg("-c")
        .arg("-f")
        .arg(".check.hvm1")
        .arg("(Main)")
        .output()
        .expect("Failed to execute command");
      (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
      )
    }
    "--ghc" => {
      // Generates the Haskell checker.
      let check_hs = generate_check_hs(&book, cmd, file);
      let mut file = File::create(".check.hs").expect("Failed to create '.check.hs'.");
      file.write_all(check_hs.as_bytes()).expect("Failed to write '.check.hs'.");

      // Calls GHC and get outputs.
      let output = Command::new("runghc").arg(".check.hs").output().expect("Failed to execute command");
      (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
      )
    }
    _ => {
      // Generates the hvm2 checker.
      let check_hs = generate_check_hvm2(&book, cmd, file);
      let mut file = File::create(".check.hvm2").expect("Failed to create '.check.hs'.");
      file.write_all(check_hs.as_bytes()).expect("Failed to write '.check.hs'.");
      
      let output = Command::new("hvml").arg("run").arg("-L").arg(".check.hvm2").output().expect("Failed to execute command");
      let stdout = &output.stdout;
      let stdout = stdout.rsplit_once(|x| *x == 0xa).map(|x| x.0).unwrap_or(b"");
      let stdout = stdout.rsplit_once(|x| *x == 0xa).map(|x| x.0).unwrap_or(b"");
      (
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
      )
    }
  };
  (Info::parse_infos(&stdout), stderr)
}

fn main() {
  let args: Vec<String> = env::args().collect();

  if args.len() < 2 {
    show_help();
  }

  let cmd = &args[1];

  //println!("loading");
  match cmd.as_str() {
    "check" | "run" => {
      if args.len() < 3 {
        show_help();
      }
      let arg = &args[2];
      let opt = &args.get(3).map(|x| x.as_str()).unwrap_or("--hvm2");
      match Book::load(".", arg) {
        Ok(book) => {
          let (infos, stderr) = get_infos(&book, cmd, arg, opt);

          for info in &infos {
            println!("{}", info.pretty(&book))
          }
          if infos.len() == 0 {
            println!("check!");
          }

          // Prints stdout errors and stats.
          eprintln!("{}", stderr);
        }
        Err(e) => {
          eprintln!("{}", e);
          std::process::exit(1);
        }
      }
    }
    "compare" => {
      test_runtime_compatibility();
    }
    _ => {
      show_help();
    }
  }
}

fn show_help() {
  eprintln!("Usage: kind2 [check|run] <name>");
  eprintln!("(or kind2 compare)");
  std::process::exit(1);
}

fn test_runtime_compatibility() {
  use std::fs;
  
  let q = env!("CARGO_MANIFEST_DIR").to_owned() + "/book/";
  let paths = fs::read_dir(&q).unwrap();
  let mut paths: Vec<_> = paths.into_iter().map(|path| path.unwrap().path()).collect();
  paths.sort();
  for path in paths {
    let base = path.file_stem().unwrap().to_str().unwrap();
    let Ok(book) = Book::load(&q, base) else { continue; };
    print!("{:<30}: ", base);
    for rt in ["--ghc", "--hvm1", "--hvm2"] {
      let _ = std::io::stdout().flush();
      let infos = get_infos(&book, "check", base, rt);
      if infos.0.len() == 0  {
        print!("\x1b[32;1m✅\x1b[0m ");
      } else {
        print!("\x1b[31;1m❎\x1b[0m ");
      }

    }
    println!();
  }
}

//fn main() {
//let rope = Rope::node(vec![
//Rope::text("Foo"),
//Rope::text("Bar"),
//Rope::node(vec![
//Rope::text("Foo"),
//Rope::text("Bar"),
//Rope::text("Tic"),
//Rope::text("Tac"),
//Rope::text("Toe"),
//]),
//Rope::text("Tac"),
//Rope::text("Toe"),
//]);

//println!("{}", rope.flatten(10));
//}
