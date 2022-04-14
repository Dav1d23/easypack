use easypack::*;
use std::env::args;

fn help() {
    eprintln!("Usage:");
    eprintln!("  pack `outfile` `data name 1` `infile 1` [...]");
    eprintln!("  unpack `infile` `data name 1` `outfile 1` [...]");
}

fn pack(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        help();
        return Err("Not enough arguments.".into());
    }
    let (outfile, args) = args.split_at(1);

    match outfile.get(0) {
        Some(outfile) => {
            if args.is_empty() || args.len() % 2 != 0 {
                help();
                return Err("Arguments must be `data name` `file` `data name` `file` ...".into());
            }

            // I can unwrap since I've already checked this is ok.
            let slice = args
                .chunks(2)
                .map(|v| (v.get(0).unwrap(), v.get(1).unwrap()));
            pack_files(outfile, slice)?;
        }
        None => return Err("`outfile` not provided".into()),
    }

    Ok(())
}

fn unpack(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        help();
        return Err("Not enough arguments.".into());
    }
    let (infile, args) = args.split_at(1);

    match infile.get(0) {
        Some(infile) => {
            if args.is_empty() || args.len() % 2 != 0 {
                help();
                return Err("Arguments must be `data name` `file` `data name` `file` ...".into());
            }

            // I can unwrap since I've already checked this is ok.
            let slice = args
                .chunks(2)
                .map(|v| (v.get(0).unwrap(), v.get(1).unwrap()));
            let unable_to_unpack = unpack_files(infile, slice)?;
            if !unable_to_unpack.is_empty() {
                eprintln!("Not found in input file:");
                for name in unable_to_unpack {
                    eprintln!("- {}", name);
                }
            }
        }
        None => return Err("`outfile` not provided".into()),
    }

    Ok(())
}

/// Offers 2 options:
/// - pack `outputfile` `data name 1` `file 1` [...]
///   The pack command accepts an outputfile and at least a couple which
///   identifies the name we want to store for this record, and the file where
///   we take the data from;
/// - unpack `inputfile` `data name 1` `file 1` [...]
///   accpepts an input (packed) file and a series of
///   `name of the input data` + `where to store it`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = args().skip(1).collect();
    if args.is_empty() {
        help();
        return Err("Not enough arguments.".into());
    }
    let (command, args) = args.split_at(1);
    match command.get(0) {
        Some(v) => match v.as_str() {
            "pack" => pack(args)?,
            "unpack" => unpack(args)?,
            _ => {
                help();
                return Err(format!("Unkown command: {}", v).into());
            }
        },
        _ => panic!("This should never happen!"),
    }

    Ok(())
}
