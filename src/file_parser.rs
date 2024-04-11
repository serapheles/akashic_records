use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};

use log::error;

//TODO: Refactor so they all use the same relevant file open, propagate error(s).

//Reads a single line from a file (or rather, reads the entire file as a string).
pub fn read_key(file_name: &str) -> String {
    fs::read_to_string(file_name)
        .unwrap_or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                error!("File not found: {}", file_name);
                panic!("File not found: {}", file_name);
            } else {
                error!("Error reading file {}: {:?}", file_name, error);
                panic!("Error reading file {}: {:?}", file_name, error);
            }
        })
        .trim()
        .to_string()
}

//Reads each line from a file as a String and separates them with ","
//Ignores lines that start with "#".
pub fn read_list(file_name: &str) -> String {
    BufReader::new(File::open(file_name).unwrap_or_else(|error| {
        if error.kind() == ErrorKind::NotFound {
            error!("File not found: {}", file_name);
            panic!("File not found: {}", file_name);
        } else {
            error!("Error reading file {}: {:?}", file_name, error);
            panic!("Error reading file {}: {:?}", file_name, error);
        }
    }))
    .lines()
    .map(|x| x.unwrap().trim().to_owned() + ",")
    .filter(|x| !x.starts_with("#"))
    .collect::<String>()
}

//Creates a hashset from the lines in a file, ignoring lines that start with "#".
//Not concerned that this could be done more directly, for what it's being used for.
pub fn read_set(file_name: &str) -> HashSet<String> {
    HashSet::from_iter(read_vec(file_name))
}

//Creates a vector from the lines in a file, ignoring lines that start with "#".
pub fn read_vec(file_name: &str) -> Vec<String> {
    BufReader::new(File::open(file_name).unwrap_or_else(|error| {
        if error.kind() == ErrorKind::NotFound {
            error!("File not found: {}", file_name);
            panic!("File not found: {}", file_name);
        } else {
            error!("Error reading file {}: {:?}", file_name, error);
            panic!("Error reading file {}: {:?}", file_name, error);
        }
    }))
    .lines()
    .map(|x| x.unwrap())
    .filter(|x| !x.starts_with("#"))
    .collect::<Vec<String>>()
}
