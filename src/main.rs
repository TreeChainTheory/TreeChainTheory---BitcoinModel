mod treechain;
use treechain::block::Block;

fn main() {
    let mut genisis_block = Block::genisis();
    Block::block_hash(&mut genisis_block);
    println!("Genisis Block: {:#?}", genisis_block);
    println!("Genisis Block Hash: {}", genisis_block.hash);
}
