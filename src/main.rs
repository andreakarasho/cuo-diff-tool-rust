use binary_reader::{BinaryReader, Endian};
use flate2::read::GzDecoder;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Error, Read, Seek, SeekFrom, Write},
    mem,
    path::Path, vec,
};

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
    uop: &Path,
    mul: &Path,
    idx: &Path,
    chunk_ids0: &HashMap<u64, usize>,
    chunk_ids1: &HashMap<u64, usize>,
) -> std::io::Result<()> {
    let mut uop_file = File::open(&uop)?;
    let mut mul_file = File::create(&mul)?;
    let mut idx_file = File::create(&idx)?;

    let mut uop_reader = BinaryReader::from_file(&mut uop_file);
    uop_reader.set_endian(Endian::Big);

    let file_type = FileType::Art;

    // let mut mul_writer = BinaryReader::from_file(&mut mul_file);
    // mul_writer.set_endian(Endian::Big);

    // let mut idx_writer = BinaryReader::from_file(&mut idx_file);
    // idx_writer.set_endian(Endian::Big);

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

            if file_type == FileType::Multi && offset.identifier == 0x126D1E99DDEDEE0A {
                let mut bin = OpenOptions::new()
                    .write(true)
                    .append(true)
                    .open("housing.bin")?;

                uop_reader.jmp((offset.offset as u64 + (offset.header_length as u64)) as usize);
                let bin_data = uop_reader.read_bytes(offset.size as usize)?;
                let mut bin_data_to_write = vec![];
                bin_data_to_write.extend_from_slice(&bin_data);

                if offset.compression == 1 {
                    bin_data_to_write.clear();
                    GzDecoder::new(bin_data).read(&mut bin_data_to_write)?;
                }

                bin.write(&bin_data_to_write)?;

                continue;
            }

            let chunk_id = chunk_ids0
                .get(&offset.identifier)
                .unwrap_or_else(|| chunk_ids1.get(&offset.identifier).unwrap());

            uop_reader.jmp((offset.offset + (offset.header_length as i64)) as usize);

            let chunk_data_raw = uop_reader.read_bytes(offset.size as usize)?;
            let mut chunk_data = vec![];
            chunk_data.extend_from_slice(chunk_data_raw);

            if offset.compression == 1 {
                chunk_data.clear();
                chunk_data.extend_from_slice(GzDecoder::new(chunk_data_raw).get_mut());
            }

            if file_type == FileType::Map {
                mul_file.seek(SeekFrom::Start((chunk_id * 0xC4000) as u64))?;
                mul_file.write(&chunk_data)?;
            } else {
                let mut data_offset = 0;

                idx_file.seek(SeekFrom::Start(*chunk_id as u64 * 12))?;
                idx_file.write(&(mul_file.stream_position()? as u32).to_be_bytes())?;

                match file_type {
                    FileType::Gump => {
                        let width = u32::from_le_bytes([
                            chunk_data[0],
                            chunk_data[1],
                            chunk_data[2],
                            chunk_data[3],
                        ]);
                        let height = u32::from_le_bytes([
                            chunk_data[4],
                            chunk_data[5],
                            chunk_data[6],
                            chunk_data[7],
                        ]);

                        idx_file.write(&(chunk_data.len() - 8).to_be_bytes())?;
                        idx_file.write(&((width << 16) | height).to_be_bytes())?;

                        data_offset = 8;
                    }
                    FileType::Sound => {
                        idx_file.write(&chunk_data.len().to_be_bytes())?;
                        idx_file.write(&(chunk_id + 1).to_be_bytes())?;
                    }
                    FileType::Multi => {
                        let mut multi_reader = BinaryReader::from_u8(&chunk_data);
                        multi_reader.set_endian(Endian::Big);

                        let mut vec = vec![];

                        _ = multi_reader.read_u32()?;
                        let count = multi_reader.read_u32()?;

                        for _ in 0..count {
                            let id = multi_reader.read_u16()?;
                            let x = multi_reader.read_i16()?;
                            let y = multi_reader.read_i16()?;
                            let z = multi_reader.read_i16()?;
                            let flags = multi_reader.read_u16()?;
                            let cliloc_count = multi_reader.read_i32()?;

                            if cliloc_count > 0 {
                                multi_reader.adv(cliloc_count as usize * mem::size_of::<u32>());
                            }

                            id.to_be_bytes().map(|s| vec.push(s));
                            x.to_be_bytes().map(|s| vec.push(s));
                            y.to_be_bytes().map(|s| vec.push(s));
                            z.to_be_bytes().map(|s| vec.push(s));
                            (match flags {
                                256u16 => 0x0000000100000001u64,
                                257u16 | 1u16 => 0u64,
                                _ => 1u64,
                            })
                            .to_be_bytes()
                            .map(|s| vec.push(s));
                        }

                        let len = mul_file.stream_position()?;
                        mul_file.seek(SeekFrom::Start(0))?;

                        chunk_data = vec![0u8; len as usize];
                        mul_file.read(&mut chunk_data)?;

                        idx_file.write(&chunk_data.len().to_be_bytes())?;
                        idx_file.write(&[0u8, 0u8, 0u8, 0u8])?;
                    }
                    _ => {
                        idx_file.write(&chunk_data.len().to_be_bytes())?;
                        idx_file.write(&[0u8, 0u8, 0u8, 0u8])?;
                    }
                }

                mul_file.write(&chunk_data[data_offset..])?;
            }
        }

        if next_table == 0 {
            break;
        }

        uop_reader.jmp(next_table as usize);
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

#[derive(Eq, PartialEq)]
enum FileType {
    Art,
    Gump,
    Map,
    Sound,
    Multi,
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
