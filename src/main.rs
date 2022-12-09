use clap::Parser;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use libc::ENOENT;
use log::debug;
use std::ffi::{OsStr, OsString};
use std::fs::{File, Metadata};
use std::os::unix::prelude::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::time::{Duration, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1); // 1 second

const DSP_DIR_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o555,
    nlink: 2,
    uid: 0,
    gid: 0,
    rdev: 0,
    blksize: 512,
    padding: 0,
    flags: 0,
};

fn make_system_time(sec: i64, nsec: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(sec as u64) + Duration::from_nanos(nsec as u64)
}
struct DspFs {
    file_name: OsString,
    file: File,
    metadata: Metadata,
}

impl DspFs {
    fn new(file_path: &Path) -> Self {
        let file = match File::open(file_path) {
            Err(why) => panic!("couldn't open {}: {}", file_path.display(), why),
            Ok(file) => file,
        };

        Self {
            file_name: file_path.file_name().unwrap().to_owned(),
            file,
            metadata: std::fs::metadata(file_path).unwrap(),
        }
    }

    fn get_file_attr(&self) -> FileAttr {
        FileAttr {
            ino: 2,
            size: self.metadata.size() * 2,
            blocks: self.metadata.blocks() * 2,
            atime: make_system_time(self.metadata.atime(), self.metadata.atime_nsec()),
            mtime: make_system_time(self.metadata.mtime(), self.metadata.mtime_nsec()),
            ctime: make_system_time(self.metadata.ctime(), self.metadata.ctime_nsec()),
            crtime: UNIX_EPOCH,
            kind: FileType::RegularFile,
            perm: 0o400,
            nlink: self.metadata.nlink() as u32,
            uid: self.metadata.uid(),
            gid: self.metadata.gid(),
            rdev: self.metadata.rdev() as u32,
            blksize: self.metadata.blksize() as u32,
            padding: 0,
            flags: 0,
        }
    }
}

impl Filesystem for DspFs {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == 1 && name == self.file_name {
            reply.entry(&TTL, &self.get_file_attr(), 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &DSP_DIR_ATTR),
            2 => reply.attr(&TTL, &self.get_file_attr()),
            _ => reply.error(ENOENT),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        inode: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        debug!(
            "read() called on {:?} offset={:?} size={:?} fh={fh:?}",
            inode, offset, size
        );
        let offset = offset / 2;
        assert!(offset >= 0);

        if inode == 2 {
            let file_size = self.file.metadata().unwrap().len();

            let read_size = std::cmp::min(size / 2, file_size.saturating_sub(offset as u64) as u32);

            let mut src_buf = vec![0; read_size as usize];
            let mut dst_buf = vec![0; 2 * read_size as usize];

            //read and convert
            self.file
                .read_exact_at(&mut src_buf, offset as u64)
                .unwrap();

            let (_, i16_buf, _) = unsafe { src_buf.align_to_mut::<i16>() };
            let (_, cf32_buf, _) = unsafe { dst_buf.align_to_mut::<f32>() };

            std::iter::zip(cf32_buf, i16_buf).for_each(|(f, i)| *f = (*i as f32) / (0x7fff as f32));

            reply.data(&dst_buf);
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let entries = vec![
            (1, FileType::Directory, OsStr::new(".")),
            (1, FileType::Directory, OsStr::new("..")),
            (2, FileType::RegularFile, self.file_name.as_os_str()),
        ];

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}

/// A FUSE FS that does on-the-fly sample type conversion of an input file.
///
/// Currently only a proof-of-concept that operates on a single file with i16
/// values and converts to f32.
#[derive(Parser)]
struct Cli {
    /// The cs16 formatted file to convert
    file: PathBuf,

    /// The FUSE mount point
    mount_point: PathBuf,
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();

    let dsp_fs = DspFs::new(&cli.file);

    // let mount_point = env::args_os().nth(1).unwrap();
    let mount_point = cli.mount_point.as_os_str();

    let options = ["-o", "ro", "-o", "fsname=fuse_dsp"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();

    fuser::mount(dsp_fs, mount_point, &options).unwrap();
}
