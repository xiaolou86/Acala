// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Mocks for idle-scheduler module.

#![cfg(test)]

use crate as module_idle_scheduler;
use acala_primitives::{define_combined_task, task::TaskResult};
use frame_support::weights::Weight;
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64, Everything},
};
use module_support::DispatchableTask;
pub use sp_runtime::offchain::storage::StorageValueRef;
use sp_runtime::BuildStorage;

use super::*;
use codec::{Decode, Encode};
use scale_info::TypeInfo;

pub const BASE_WEIGHT: Weight = Weight::from_parts(1_000_000, 0);
pub const RELAY_BLOCK_KEY: [u8; 32] = [0; 32];

pub type AccountId = u32;
impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type RuntimeCall = RuntimeCall;
	type Hash = sp_runtime::testing::H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

pub struct MockBlockNumberProvider;

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u32;

	fn current_block_number() -> Self::BlockNumber {
		// gets a local mock storage value
		u32::decode(&mut &sp_io::storage::get(&RELAY_BLOCK_KEY).unwrap()[..]).unwrap()
	}
}

parameter_types! {
	pub MinimumWeightRemainInBlock: Weight = Weight::from_parts(100_000_000_000, 0);
}

impl module_idle_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = MockBlockNumberProvider;
	type DisableBlockThreshold = ConstU32<6>;
}

// Mock dispatachable tasks
#[derive(Clone, Debug, PartialEq, Encode, Decode, TypeInfo)]
pub enum BalancesTask {
	#[codec(index = 0)]
	OnIdle,
}
impl DispatchableTask for BalancesTask {
	fn dispatch(self, weight: Weight) -> TaskResult {
		TaskResult {
			result: Ok(()),
			used_weight: BASE_WEIGHT,
			finished: weight.ref_time() >= BASE_WEIGHT.ref_time(),
		}
	}
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, TypeInfo)]
pub enum HomaLiteTask {
	#[codec(index = 0)]
	OnIdle,
}
impl DispatchableTask for HomaLiteTask {
	fn dispatch(self, weight: Weight) -> TaskResult {
		TaskResult {
			result: Ok(()),
			used_weight: BASE_WEIGHT,
			finished: weight.ref_time() >= BASE_WEIGHT.ref_time(),
		}
	}
}

define_combined_task! {
	#[derive(Clone, Debug, PartialEq, Encode, Decode, TypeInfo)]
	pub enum ScheduledTasks {
		BalancesTask(BalancesTask),
		HomaLiteTask(HomaLiteTask),
	}
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		IdleScheduler: module_idle_scheduler,
	}
);

#[derive(Default)]
pub struct ExtBuilder;
impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext.execute_with(|| sp_io::storage::set(&RELAY_BLOCK_KEY, &0_u32.encode()));
		ext
	}
}
