use binary_reader::{BinaryReader, Endian};
use flate2::bufread::ZlibDecoder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Error, Read, Seek, SeekFrom, Write},
    mem,
    path::Path,
    time::Instant,
    vec,
};

fn main() {
    println!("start patching...");
    let start = Instant::now();

    let files = [
        "artLegacyMUL.uop",
        "gumpartLegacyMUL.uop",
        "MultiCollection.uop",
        "soundLegacyMUL.uop",
        "map0LegacyMUL.uop",
        "map1LegacyMUL.uop",
        "map2LegacyMUL.uop",
        "map3LegacyMUL.uop",
        "map4LegacyMUL.uop",
        "map5LegacyMUL.uop",
    ];

    for f in files.iter() {
        println!("running {}", &f);

        patch(PatchArgs {
            source_dir: String::from("D:\\Giochi\\Ultima Online Classic"),
            output_dir: String::from("./output"),
            file_to_process: String::from(*f),
        })
        .unwrap();
    }

    let duration = start.elapsed();
    println!("Time elapsed is: {:?}", duration);
}

fn patch(args: PatchArgs) -> std::io::Result<()> {
    let output_path = Path::new(&args.output_dir);

    if !output_path.exists() {
        std::fs::create_dir_all(&output_path)?;
    }

    uop_to_mul(&args)?;

    Ok(())
}

fn uop_to_mul(args: &PatchArgs) -> std::io::Result<()> {
    let descriptor = get_file_descriptor(&args.file_to_process);

    let mut uop_file = File::open(&Path::new(&args.source_dir).join(&args.file_to_process))?;
    let mut mul_file = File::create(&Path::new(&args.output_dir).join(&descriptor.mul))?;
    let mut idx_file_maybe = File::create(&Path::new(&args.output_dir).join(&descriptor.idx));

    let mut uop_reader = BinaryReader::from_file(&mut uop_file);
    uop_reader.set_endian(Endian::Little);

    let hashes: HashMap<u64, usize> = descriptor
        .uop_patterns
        .iter()
        .enumerate()
        .map(|(i, s)| (hash_little_2(&s), i))
        .collect();

    let magic = uop_reader.read_u32()?;
    if magic != 0x50594D {
        return Err(Error::new(std::io::ErrorKind::Other, "invalid UOP file"));
    }

    let _version = uop_reader.read_i32()?;
    let _timestamp = uop_reader.read_i32()?;
    let mut next_table = uop_reader.read_i64()?;

    loop {
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

            if descriptor.file_type == FileType::Multi && offset.identifier == 0x126D1E99DDEDEE0A {
                let housing_path = Path::new("./housing.bin");

                if housing_path.exists() {
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
                        ZlibDecoder::new(bin_data).read_to_end(&mut bin_data_to_write)?;
                    }

                    bin.write(&bin_data_to_write)?;
                }

                continue;
            }

            if let Some(chunk_id) = hashes.get(&offset.identifier) {
                uop_reader.jmp((offset.offset + (offset.header_length as i64)) as usize);

                let chunk_data_raw = uop_reader.read_bytes(offset.size as usize)?;
                let mut chunk_data = vec![];
                chunk_data.extend_from_slice(chunk_data_raw);

                if offset.compression == 1 {
                    chunk_data.clear();
                    ZlibDecoder::new(chunk_data_raw).read_to_end(&mut chunk_data)?;
                }

                if descriptor.file_type == FileType::Map {
                    mul_file.seek(SeekFrom::Start((chunk_id * 0xC4000) as u64))?;
                    mul_file.write(&chunk_data)?;
                } else if let Ok(idx_file) = idx_file_maybe.as_mut() {
                    let mut data_offset = 0;

                    idx_file.seek(SeekFrom::Start(*chunk_id as u64 * 12))?;
                    idx_file.write(&(mul_file.stream_position()? as u32).to_le_bytes())?;

                    match descriptor.file_type {
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

                            idx_file.write(&(chunk_data.len() - 8).to_le_bytes())?;
                            idx_file.write(&((width << 16) | height).to_le_bytes())?;

                            data_offset = 8;
                        }
                        FileType::Sound => {
                            idx_file.write(&chunk_data.len().to_le_bytes())?;
                            idx_file.write(&(chunk_id + 1).to_le_bytes())?;
                        }
                        FileType::Multi => {
                            let mut multi_reader = BinaryReader::from_u8(&chunk_data);
                            multi_reader.set_endian(Endian::Little);

                            chunk_data.clear();

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

                                chunk_data.extend(id.to_le_bytes());
                                chunk_data.extend(x.to_le_bytes());
                                chunk_data.extend(y.to_le_bytes());
                                chunk_data.extend(z.to_le_bytes());
                                chunk_data.extend(
                                    (match flags {
                                        256u16 => 0x0000000100000001u64,
                                        257u16 | 1u16 => 0u64,
                                        _ => 1u64,
                                    })
                                    .to_le_bytes(),
                                );
                            }

                            idx_file.write(&(chunk_data.len() as i32).to_le_bytes())?;
                            idx_file.write(&[0u8, 0u8, 0u8, 0u8])?;
                        }
                        _ => {
                            idx_file.write(&(chunk_data.len() as i32).to_le_bytes())?;
                            idx_file.write(&[0u8, 0u8, 0u8, 0u8])?;
                        }
                    }

                    mul_file.write(&chunk_data[data_offset..])?;
                }
            }
        }

        if next_table == 0 {
            break;
        }

        uop_reader.jmp(next_table as usize);
    }

    Ok(())
}

fn get_file_descriptor(uop_file: &String) -> FileDescriptor {
    let (mul_name, idx_name, file_type, type_index) = get_uop_mul_name(&uop_file).unwrap();

    const MAX_ID: i32 = 0x7FFFF;

    let (pattern0, pattern1, max_index) = match file_type {
        FileType::Art => (
            (0..0x13FDC)
                .into_iter()
                .map(|i| String::from(format!("build/artlegacymul/{:08}.tga", i)))
                .collect::<Vec<String>>(),
            Vec::<String>::new(),
            0x13FDC,
        ),
        FileType::Gump => (
            (0..MAX_ID)
                .into_iter()
                .map(|i| String::from(format!("build/gumpartlegacymul/{:08}.tga", i)))
                .collect(),
            (0..MAX_ID)
                .into_iter()
                .map(|i| String::from(format!("build/gumpartlegacymul/{:07}.tga", i)))
                .collect(),
            MAX_ID,
        ),
        FileType::Map => (
            (0..MAX_ID)
                .into_iter()
                .map(|i| String::from(format!("build/map{}legacymul/{:08}.dat", type_index, i)))
                .collect(),
            Vec::<String>::new(),
            MAX_ID,
        ),
        FileType::Sound => (
            (0..MAX_ID)
                .into_iter()
                .map(|i| String::from(format!("build/soundlegacymul/{:08}.dat", i)))
                .collect(),
            Vec::<String>::new(),
            MAX_ID,
        ),
        FileType::Multi => (
            (0..u16::MAX as i32)
                .into_iter()
                .map(|i| String::from(format!("build/multicollection/{:06}.bin", i)))
                .collect(),
            Vec::<String>::new(),
            u16::MAX as i32,
        ),
    };

    let mut all_patterns: Vec<String> = Vec::new();
    all_patterns.extend(pattern0);
    all_patterns.extend(pattern1);

    FileDescriptor {
        uop: uop_file.clone(),
        uop_patterns: all_patterns,
        max_index: max_index,
        mul: mul_name,
        idx: idx_name,
        file_type: file_type,
    }
}

fn get_uop_mul_name(uop_file: &String) -> Option<(String, String, FileType, i32)> {
    match uop_file.as_str() {
        "artLegacyMUL.uop" => Some((
            String::from("art.mul"),
            String::from("artidx.mul"),
            FileType::Art,
            -1,
        )),
        "gumpartLegacyMUL.uop" => Some((
            String::from("gumpart.mul"),
            String::from("gumpidx.mul"),
            FileType::Gump,
            -1,
        )),
        "MultiCollection.uop" => Some((
            String::from("multi.mul"),
            String::from("multi.idx"),
            FileType::Multi,
            -1,
        )),
        "soundLegacyMUL.uop" => Some((
            String::from("sound.mul"),
            String::from("soundidx.mul"),
            FileType::Sound,
            -1,
        )),
        _ => {
            let re = Regex::new(r"^map(\d+)LegacyMUL.uop$").unwrap();
            if let Some(cap) = re.captures(uop_file) {
                let num_str = cap.get(1).unwrap().as_str();
                let num = num_str.parse::<i32>().ok().unwrap();
                return Some((
                    String::from(format!("map{}.mul", num)),
                    String::from(""),
                    FileType::Map,
                    num,
                ));
            }

            None
        }
    }
}

fn hash_little_2(s: &str) -> u64 {
    let mut length = s.len();

    let mut a = 0xDEADBEEF + length as u32;
    let mut b = 0xDEADBEEF + length as u32;
    let mut c = 0xDEADBEEF + length as u32;

    let mut k = 0;

    while length > 12 {
        a = a.wrapping_add(s.as_bytes()[k] as u32);
        a = a.wrapping_add((s.as_bytes()[k + 1] as u32) << 8);
        a = a.wrapping_add((s.as_bytes()[k + 2] as u32) << 16);
        a = a.wrapping_add((s.as_bytes()[k + 3] as u32) << 24);
        b = b.wrapping_add(s.as_bytes()[k + 4] as u32);
        b = b.wrapping_add((s.as_bytes()[k + 5] as u32) << 8);
        b = b.wrapping_add((s.as_bytes()[k + 6] as u32) << 16);
        b = b.wrapping_add((s.as_bytes()[k + 7] as u32) << 24);
        c = c.wrapping_add(s.as_bytes()[k + 8] as u32);
        c = c.wrapping_add((s.as_bytes()[k + 9] as u32) << 8);
        c = c.wrapping_add((s.as_bytes()[k + 10] as u32) << 16);
        c = c.wrapping_add((s.as_bytes()[k + 11] as u32) << 24);

        a = a.wrapping_sub(c);
        a ^= (c << 4) | (c >> 28);

        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= (a << 6) | (a >> 26);

        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= (b << 8) | (b >> 24);

        b = b.wrapping_add(a);
        a = a.wrapping_sub(c);
        a ^= (c << 16) | (c >> 16);

        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= (a << 19) | (a >> 13);

        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= (b << 4) | (b >> 28);

        b = b.wrapping_add(a);

        length -= 12;
        k += 12;
    }

    if length != 0 {
        let mut remains = length;

        while remains > 0 {
            match remains {
                12 => {
                    c = c.wrapping_add((s.as_bytes()[k + 11] as u32) << 24);
                    // fallthrough
                }
                11 => {
                    c = c.wrapping_add((s.as_bytes()[k + 10] as u32) << 16);
                    // fallthrough
                }
                10 => {
                    c = c.wrapping_add((s.as_bytes()[k + 9] as u32) << 8);
                    // fallthrough
                }
                9 => {
                    c = c.wrapping_add(s.as_bytes()[k + 8] as u32);
                    // fallthrough
                }
                8 => {
                    b = b.wrapping_add((s.as_bytes()[k + 7] as u32) << 24);
                    // fallthrough
                }
                7 => {
                    b = b.wrapping_add((s.as_bytes()[k + 6] as u32) << 16);
                    // fallthrough
                }
                6 => {
                    b = b.wrapping_add((s.as_bytes()[k + 5] as u32) << 8);
                    // fallthrough
                }
                5 => {
                    b = b.wrapping_add(s.as_bytes()[k + 4] as u32);
                    // fallthrough
                }
                4 => {
                    a = a.wrapping_add((s.as_bytes()[k + 3] as u32) << 24);
                    // fallthrough
                }
                3 => {
                    a = a.wrapping_add((s.as_bytes()[k + 2] as u32) << 16);
                    // fallthrough
                }
                2 => {
                    a = a.wrapping_add((s.as_bytes()[k + 1] as u32) << 8);
                    // fallthrough
                }
                1 => {
                    a = a.wrapping_add(s.as_bytes()[k] as u32);
                }
                _ => unreachable!(),
            }

            remains -= 1;
        }

        c ^= b;
        c = c.wrapping_sub((b << 14) | (b >> 18));

        a ^= c;
        a = a.wrapping_sub((c << 11) | (c >> 21));

        b ^= a;
        b = b.wrapping_sub((a << 25) | (a >> 7));

        c ^= b;
        c = c.wrapping_sub((b << 16) | (b >> 16));

        a ^= c;
        a = a.wrapping_sub((c << 4) | (c >> 28));

        b ^= a;
        b = b.wrapping_sub((a << 14) | (a >> 18));

        c ^= b;
        c = c.wrapping_sub((b << 24) | (b >> 8));
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

struct FileDescriptor {
    uop: String,
    uop_patterns: Vec<String>,
    max_index: i32,
    mul: String,
    idx: String,
    file_type: FileType,
}

#[derive(Serialize, Deserialize, Debug)]
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
    output_dir: String,
    file_to_process: String,
}
