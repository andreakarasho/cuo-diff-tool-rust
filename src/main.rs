use binary_reader::{BinaryReader, Endian};
use flate2::read::GzDecoder;
use std::{collections::HashMap, fs::File, io::{Error, Seek, SeekFrom, Write}, path::Path};

fn main() {
    println!("Hello, world!");
}

fn patch(args: PatchArgs) -> std::io::Result<()> {
    let output_path = Path::new(&args.output_dir);

    if !output_path.exists() {
        std::fs::create_dir_all(&output_path)?;
    }

    if let Some(mul) = get_uop_mul_name(&args.file_to_process) {
        //uop_to_mul(&mul);
    }

    Ok(())
}

fn uop_to_mul(
    uop: &String,
    chunk_ids0: &HashMap<u64, usize>,
    chunk_ids1: &HashMap<u64, usize>,
) -> std::io::Result<()> {
    let mut mul_file = File::create("")?;
    let mut idx_file = File::create("")?;
    let mut uop_file = File::open(uop)?;
  
    let mut uop_reader = BinaryReader::from_file(&mut uop_file);
    uop_reader.set_endian(Endian::Big);

    let mut mul_writer = BinaryReader::from_file(&mut mul_file);
    mul_writer.set_endian(Endian::Big);

    let mut idx_writer = BinaryReader::from_file(&mut idx_file);
    idx_writer.set_endian(Endian::Big);
    

    let magic = uop_reader.read_u32()?;
    if magic != 0x50594D {
        return Err(Error::new(std::io::ErrorKind::Other, "invalid UOP file"));
    }

    let _version = uop_reader.read_i32()?;
    let _timestamp = uop_reader.read_i32()?;
    let mut next_table = uop_reader.read_i64()?;

    loop {
        // might not work
        uop_reader.jmp(next_table as usize);

        let entries_count = uop_reader.read_i32()?;
        next_table = uop_reader.read_i64()?;

        let mut offsets = vec![];

        for _ in 0..entries_count {
            offsets.push(TableEntry {
                offset: uop_reader.read_i64()?,
                header_length: uop_reader.read_i32()?,
                size: uop_reader.read_i32()?,
                size_decompressed: uop_reader.read_i32()?,
                identifier: uop_reader.read_u64()?,
                hash: uop_reader.read_u32()?,
                compression: uop_reader.read_i16()?,
            })
        }

        for offset in offsets.iter() {
            if offset.offset == 0 {
                continue;
            }

            // if type -> multi
            //let chunk_id: Option<&usize>;
            // let mut chunk_id: &usize;
            // if let chunk_id = chunk_ids0.get(&offset.identifier) {
            // } else if let Some(temp_chunk_id) = chunk_ids1.get(&offset.identifier) {
            //     chunk_id = temp_chunk_id;
            // } else {
            //     return Err(Error::new(std::io::ErrorKind::Other, "chunk id not found"));
            // }

            let chunk_id = chunk_ids0.get(&offset.identifier)
                      .unwrap_or_else(|| chunk_ids1.get(&offset.identifier).unwrap());

            uop_reader.jmp((offset.offset + (offset.header_length as i64)) as usize);
            
            let chunk_data_raw = uop_reader.read_bytes(offset.size as usize)?;
            let mut chunk_data = chunk_data_raw;
            if offset.compression == 1 {
                chunk_data = GzDecoder::new(chunk_data_raw).get_mut();
            }
            
            // if type == map
            // else
            idx_file.seek(SeekFrom::Start(*chunk_id as u64 * 12))?;
            idx_file.write(mul_file.stream_position().unwrap())?;
        }

        if next_table == 0 {
            break;
        }
    }

    Ok(())
}

fn create_hashes(hash_pattern: &String, maxId: usize) -> HashMap<u64, usize> {
    let mut map = HashMap::new();

    for i in 0..maxId {
        map.insert(hash_little2(hash_pattern), i);
    }

    map
}

fn get_uop_mul_name(uop_file: &String) -> Option<String> {
    match uop_file.as_str() {
        "artLegacyMUL" => Some(String::from("art")),
        "gumpartLegacyMUL" => Some(String::from("gumpart")),
        "MultiCollection" => Some(String::from("multi")),
        "soundLegacyMUL" => Some(String::from("sound")),
        "map0LegacyMUL" => Some(String::from("map")),
        _ => None,
    }
}

fn hash_little2(s: &str) -> u64 {
    let mut length = s.len();

    let mut a: u32 = 0xDEADBEEF + length as u32;
    let mut b: u32 = 0xDEADBEEF + length as u32;
    let mut c: u32 = 0xDEADBEEF + length as u32;

    let mut k = 0usize;

    while length > 12 {
        let mut chunks = s[k..].as_bytes().chunks_exact(4);

        for chunk in &mut chunks {
            let word = u32::from_le_bytes(chunk.try_into().unwrap());

            a = a.wrapping_add(word);
            b = b.wrapping_add(word);
            c = c.wrapping_add(word);

            a = a.wrapping_sub(c);
            a ^= c.rotate_left(4);
            c = c.wrapping_add(b);
            b = b.wrapping_sub(a);
            b ^= a.rotate_left(6);
            a = a.wrapping_add(c);
            c = c.wrapping_sub(b);
            c ^= b.rotate_left(8);
            b = b.wrapping_add(a);
            a = a.wrapping_sub(c);
            a ^= c.rotate_left(16);
            c = c.wrapping_add(b);
            b = b.wrapping_sub(a);
            b ^= a.rotate_left(19);
            a = a.wrapping_add(c);
            c = c.wrapping_sub(b);
            c ^= b.rotate_left(4);
            b = b.wrapping_add(a);
        }

        k += chunks.remainder().len();
        length -= chunks.len();
    }

    if length != 0 {
        let mut remainder = s[k..].as_bytes();

        remainder = match remainder.len() {
            12 => {
                c = c.wrapping_add(u32::from_le_bytes(remainder[8..].try_into().unwrap()) << 24);
                &remainder[..8]
            }
            11 => {
                c = c.wrapping_add(
                    u32::from_le_bytes(remainder[8..].try_into().unwrap()) << 24
                        | u32::from_le_bytes([remainder[7], 0, 0, 0]) << 16,
                );
                &remainder[..7]
            }
            10 => {
                c = c.wrapping_add(
                    u32::from_le_bytes(remainder[8..].try_into().unwrap()) << 24
                        | u32::from_le_bytes([remainder[7], remainder[6], 0, 0]) << 8,
                );
                &remainder[..6]
            }
            9 => {
                c = c.wrapping_add(
                    u32::from_le_bytes(remainder[8..].try_into().unwrap()) << 24
                        | u32::from_le_bytes([remainder[7], remainder[6], remainder[5], 0]),
                );
                &remainder[..5]
            }
            8 => {
                b = b.wrapping_add(u32::from_le_bytes(remainder[4..].try_into().unwrap()) << 24);
                &remainder[..4]
            }
            7 => {
                b = b.wrapping_add(
                    u32::from_le_bytes(remainder[4..].try_into().unwrap()) << 24
                        | u32::from_le_bytes([remainder[3], 0, 0, 0]) << 16,
                );
                &remainder[..3]
            }
            6 => {
                b = b.wrapping_add(
                    u32::from_le_bytes(remainder[4..].try_into().unwrap()) << 24
                        | u32::from_le_bytes([remainder[3], remainder[2], 0, 0]) << 8,
                );
                &remainder[..2]
            }
            5 => {
                b = b.wrapping_add(
                    u32::from_le_bytes(remainder[4..].try_into().unwrap()) << 24
                        | u32::from_le_bytes([remainder[3], remainder[2], remainder[1], 0]),
                );
                &remainder[..1]
            }
            4 => {
                a = a.wrapping_add(u32::from_le_bytes(remainder.try_into().unwrap()));
                &[]
            }
            3 => {
                a = a.wrapping_add(u32::from_le_bytes([
                    remainder[2],
                    remainder[1],
                    remainder[0],
                    0,
                ]));
                &[]
            }
            2 => {
                a = a.wrapping_add(u32::from_le_bytes([remainder[1], remainder[0], 0, 0]));
                &[]
            }
            1 => {
                a = a.wrapping_add(u32::from_le_bytes([remainder[0], 0, 0, 0]));
                &[]
            }
            _ => remainder,
        };

        c ^= b;
        c = c.wrapping_sub(b.rotate_left(14));
        a ^= c;
        a = a.wrapping_sub(c.rotate_left(11));
        b ^= a;
        b = b.wrapping_sub(a.rotate_left(25));
        c ^= b;
        c = c.wrapping_sub(b.rotate_left(16));
        a ^= c;
        a = a.wrapping_sub(c.rotate_left(4));
        b ^= a;
        b = b.wrapping_sub(a.rotate_left(14));
        c ^= b;
        c = c.wrapping_sub(b.rotate_left(24));

        for chunk in remainder.chunks_exact(4) {
            let word = u32::from_le_bytes(chunk.try_into().unwrap());

            a = a.wrapping_add(word);
            b = b.wrapping_add(word);
            c = c.wrapping_add(word);

            a = a.wrapping_sub(c);
            a ^= c.rotate_left(4);
            c = c.wrapping_add(b);
            b = b.wrapping_sub(a);
            b ^= a.rotate_left(6);
            a = a.wrapping_add(c);
            c = c.wrapping_sub(b);
            c ^= b.rotate_left(8);
            b = b.wrapping_add(a);
            a = a.wrapping_sub(c);
            a ^= c.rotate_left(16);
            c = c.wrapping_add(b);
            b = b.wrapping_sub(a);
            b ^= a.rotate_left(19);
            a = a.wrapping_add(c);
            c = c.wrapping_sub(b);
            c ^= b.rotate_left(4);
            b = b.wrapping_add(a);
        }
    }

    ((b as u64) << 32) | (c as u64)
}

struct TableEntry {
    offset: i64,
    header_length: i32,
    size: i32,
    size_decompressed: i32,
    identifier: u64,
    hash: u32,
    compression: i16,
}

struct PatchArgs {
    source_dir: String,
    target_dir: String,
    output_dir: String,
    file_to_process: String,
}
