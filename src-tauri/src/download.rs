use bincode::{config, error::DecodeError, error::EncodeError, Decode, Encode};
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::path::BaseDirectory;
use tauri::Manager;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::config::InstanceConfig;

const PHI: f32 = 1.618033988749895;
// 2504730781958 to 2199023255552 for 64 bit limit
// based on 2^64/2^20/8
const RANGE: [std::ops::Range<usize>; 59] = [
    0..1,
    1..2,
    2..4,
    4..7,
    7..12,
    12..20,
    20..33,
    33..54,
    54..88,
    88..143,
    143..232,
    232..376,
    376..609,
    609..986,
    986..1596,
    1596..2583,
    2583..4180,
    4180..6764,
    6764..10945,
    10945..17710,
    17710..28656,
    28656..46367,
    46367..75024,
    75024..121392,
    121392..196417,
    196417..317810,
    317810..514228,
    514228..832039,
    832039..1346268,
    1346268..2178308,
    2178308..3524577,
    3524577..5702886,
    5702886..9227464,
    9227464..14930351,
    14930351..24157816,
    24157816..39088168,
    39088168..63245985,
    63245985..102334154,
    102334154..165580140,
    165580140..267914295,
    267914295..433494436,
    433494436..701408732,
    701408732..1134903169,
    1134903169..1836311902,
    1836311902..2971215072,
    2971215072..4807526975,
    4807526975..7778742048,
    7778742048..12586269024,
    12586269024..20365011073,
    20365011073..32951280098,
    32951280098..53316291172,
    53316291172..86267571271,
    86267571271..139583862444,
    139583862444..225851433716,
    225851433716..365435296161,
    365435296161..591286729878,
    591286729878..956722026040,
    956722026040..1548008755918,
    1548008755918..2199023255552,
];

struct Index {
    start: AtomicUsize,
    end: AtomicUsize,
}

impl Encode for Index {
    fn encode<E: bincode::enc::Encoder>(&self, e: &mut E) -> Result<(), EncodeError> {
        self.start.load(Ordering::Relaxed).encode(e)?;
        self.end.load(Ordering::Relaxed).encode(e)
    }
}

impl<Context> Decode<Context> for Index {
    fn decode<D: bincode::de::Decoder<Context = Context>>(d: &mut D) -> Result<Self, DecodeError> {
        Ok(Index {
            start: AtomicUsize::new(usize::decode(d)?),
            end: AtomicUsize::new(usize::decode(d)?),
        })
    }
}

#[derive(Encode, Decode)]
struct Coordinator {
    range_byte: Range<u8>,
    // steal_ptr: u8,  // runs in it's task so it create that on runtime
}
impl Coordinator {
    fn new(max_index: u8) -> Self {
        Coordinator {
            range_byte: 0..max_index,
            // steal_ptr: 0,
        }
    }
    // ask from coordinator, return a range
    fn new_range(&self) -> Range<usize> {
        if self.range_byte.start < self.range_byte.end {
        } else if self.range_byte.start == self.range_byte.end {
            // TODO for the case of index 364..609 but if we need till 512 or something that's less than index value than select total size helps decide
        }

        0..0
    }
}

pub struct Download {
    // id: Uuid,
    coordinator: Coordinator,
    range: VecDeque<Arc<Index>>,
}

impl Encode for Download {
    fn encode<E: bincode::enc::Encoder>(&self, e: &mut E) -> Result<(), EncodeError> {
        self.coordinator.encode(e)?;
        self.range.len().encode(e)?;
        for i in &self.range {
            i.encode(e)?
        }
        Ok(())
    }
}

impl<Context> Decode<Context> for Download {
    fn decode<D: bincode::de::Decoder<Context = Context>>(d: &mut D) -> Result<Self, DecodeError> {
        let coordinator = Coordinator::decode(d)?;
        let len = usize::decode(d)?;
        let mut range = VecDeque::with_capacity(len);
        for _ in 0..len {
            range.push_back(Arc::new(Index::decode(d)?));
        }
        Ok(Download {
            // id: Uuid::nil(),
            coordinator,
            range,
        })
    }
}

impl Download {
    pub fn new(id: Uuid, size: usize, num_conn: u8) -> Self {
        Download {
            range: VecDeque::with_capacity((PHI * num_conn as f32).round() as usize),
            coordinator: Coordinator::new(Self::get_index(size >> 23).unwrap()),
        }
    }
    /// frontend req. from History to start instance
    /// Load self from the given UUID, used when started from History
    /// let mut a = A::load(&handle, uuid).unwrap();
    pub fn load<R: tauri::Runtime>(
        handle: &tauri::AppHandle<R>,
        id: Uuid,
    ) -> Result<Self, bincode::error::DecodeError> {
        let mut file = std::fs::File::open(Self::meta_path(handle, &id)).map_err(|e| {
            bincode::error::DecodeError::Io {
                inner: e,
                additional: 0,
            }
        })?;
        let instance: Download = bincode::decode_from_std_read(&mut file, config::standard())?;
        // instance.id = id;
        Ok(instance)
    }

    /// Save self under the given ID, used for drop and pause/cancel action
    fn meta_path<R: tauri::Runtime>(
        handle: &tauri::AppHandle<R>,
        uuid: &Uuid,
    ) -> std::path::PathBuf {
        let mut p = handle
            .path()
            .resolve("metadata", BaseDirectory::AppData)
            .expect("cannot resolve AppData/metadata");
        std::fs::create_dir_all(&p).ok();
        p.push(format!("{}.tur", uuid.as_simple()));
        p
    }

    /// save to metadata path
    pub fn save<R: tauri::Runtime>(
        &self,
        handle: &tauri::AppHandle<R>,
        id: &Uuid,
    ) -> Result<(), bincode::error::EncodeError> {
        let mut file = std::fs::File::create(Self::meta_path(handle, id))
            .map_err(|e| bincode::error::EncodeError::Io { inner: e, index: 0 })?;
        bincode::encode_into_std_write(self, &mut file, config::standard()).map(|_| ())
    }

    // pass value as (value/2^20/8) or simply (value >> 23)
    pub fn get_index(v: usize) -> Option<u8> {
        let mut lo = if v <= RANGE[13].start { 0 } else { 13 };
        let mut hi = if v <= RANGE[13].start { 12 } else { 59 };

        while lo < hi {
            let mid = (lo + hi) >> 1;
            if RANGE[mid].start < v {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        (lo < RANGE.len()).then_some(lo as u8)
    }

    pub async fn work(){
        let client = reqwest::Client::new();
        let res = client.get("https://example.com").send();
        let resp = res.await.unwrap();
    }
    
    fn run_instance<R: tauri::Runtime>(handle: &tauri::AppHandle<R>, config: InstanceConfig) -> Vec<JoinHandle<()>> {
        // TODO: Read from tauri store instead of hardcoding
        // the solution: InstanceConfig is available

        // Create coordination channel
        // TODO do we need onshot or could we emit/listen to same ID
        let (tx, rx) = mpsc::channel::<oneshot::Sender<Arc<Index>>>(config.download.socket_buffer_size); // Increased buffer for better throughput

        let mut handles = Vec::new();

        // Spawn coordinator task
        // TODO creating steal_ptr in thread start and the keep using that with modulo
        let coordinator_handle = handle.clone();
        handles.push(tokio::spawn(async move {
            // TODO: Implement coordinator logic with rx and coordinator_handle
        }));

        // Spawn worker tasks
        for i in 0..config.download.num_threads {
            let worker_tx = tx.clone();
            let worker_handle = handle.clone();
            handles.push(tokio::spawn(async move {
                // TODO: Implement worker logic with worker_tx and worker_handle
                // Worker creates oneshot, keeps rx, sends tx via mpsc when needed
                // emit progress as well
            }));
        }

        handles
    }
    // db conn is on DM, it save the necessary info, DState goes to file-dl.tur
}

// destructor for cleanup and store state
// impl Drop for Download {
//     fn drop(&mut self) {
//         if let Err(e) = self.save() {
//             eprintln!("download metadata save failed: {}", e);
//         }
//     }
// }
