pub const CHILDREN: u8 = 3;
pub const BITS: &str = "1e1ffff0"; //1e1ffff0 // 207fffff Easy  1e0fffff 100xHard 1e7fffff 40xHard 1f0fffff 10xHard
pub const MINING_RATE: i32 = 100_000;
// donot put this over max 4,294,967,295 as it overflows u32 of queue_index and misbehaves
pub const EXPECTED_TIME: i128 = 10_000_000; // 10000s in ms -> adjust this according to CHILDREN * (MINING_RATE/EXPECTED_TIME) blocks (like expect CHILDREN*100 blocks in 10000s )
pub const INITIAL_SUBSIDY: u64 = 50 * 100_000_000; //50 BTC
pub const HALVING_INTERVAL: u64 = 100;
pub const INVMESSAGE_LIMIT: u16 = 40;
pub const GETDATA_LIMIT: u16 = 20;
pub const TESTING_WALLET_BALANCE: u64 = 500;
pub const USER_TXN_FREERATE: u64 = 3;
pub const SIGHASH_ALL: u32 = 0x01;
