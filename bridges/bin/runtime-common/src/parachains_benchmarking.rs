// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Parity Bridges Common.

// Parity Bridges Common is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity Bridges Common is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity Bridges Common.  If not, see <http://www.gnu.org/licenses/>.

//! Everything required to run benchmarks of parachains finality module.

#![cfg(feature = "runtime-benchmarks")]

use crate::messages_benchmarking::insert_header_to_grandpa_pallet;

use bp_parachains::{
	parachain_head_storage_key_at_source, RelayBlockHash, RelayBlockHasher, RelayBlockNumber,
};
use bp_polkadot_core::parachains::{ParaHash, ParaHead, ParaHeadsProof, ParaId};
use bp_runtime::{grow_storage_value, record_all_trie_keys, Chain, UnverifiedStorageProofParams};
use codec::Encode;
use frame_support::traits::Get;
use sp_std::prelude::*;
use sp_trie::{trie_types::TrieDBMutBuilderV1, LayoutV1, MemoryDB};

/// Prepare proof of messages for the `receive_messages_proof` call.
///
/// In addition to returning valid messages proof, environment is prepared to verify this message
/// proof.
pub fn prepare_parachain_heads_proof<R, PI>(
	parachains: &[ParaId],
	parachain_head_size: u32,
	proof_params: UnverifiedStorageProofParams,
) -> (RelayBlockNumber, RelayBlockHash, ParaHeadsProof, Vec<(ParaId, ParaHash)>)
where
	R: pallet_bridge_parachains::Config<PI>
		+ pallet_bridge_grandpa::Config<R::BridgesGrandpaPalletInstance>,
	PI: 'static,
	<R as pallet_bridge_grandpa::Config<R::BridgesGrandpaPalletInstance>>::BridgedChain:
		Chain<BlockNumber = RelayBlockNumber, Hash = RelayBlockHash>,
{
	let parachain_head = ParaHead(vec![0u8; parachain_head_size as usize]);

	// insert all heads to the trie
	let mut parachain_heads = Vec::with_capacity(parachains.len());
	let mut storage_keys = Vec::with_capacity(parachains.len());
	let mut mdb = MemoryDB::default();
	let mut trie = TrieDBMutBuilderV1::<RelayBlockHasher>::new(&mdb).build();

	// insert parachain heads
	for (i, parachain) in parachains.into_iter().enumerate() {
		let storage_key =
			parachain_head_storage_key_at_source(R::ParasPalletName::get(), *parachain);
		let leaf_data = if i == 0 {
			grow_storage_value(parachain_head.encode(), &proof_params)
		} else {
			parachain_head.encode()
		};
		trie.insert(&storage_key.0, &leaf_data)
			.map_err(|_| "TrieDBMut::insert has failed")
			.expect("TrieDBMut::insert should not fail in benchmarks");
		storage_keys.push(storage_key);
		parachain_heads.push((*parachain, parachain_head.hash()))
	}

	let state_root = trie.commit().apply_to(&mut mdb);

	// generate heads storage proof
	let proof = record_all_trie_keys::<LayoutV1<RelayBlockHasher, ()>>(&mdb, &state_root)
		.map_err(|_| "record_all_trie_keys has failed")
		.expect("record_all_trie_keys should not fail in benchmarks");

	let (relay_block_number, relay_block_hash) =
		insert_header_to_grandpa_pallet::<R, R::BridgesGrandpaPalletInstance>(state_root);

	(relay_block_number, relay_block_hash, ParaHeadsProof { storage_proof: proof }, parachain_heads)
}
