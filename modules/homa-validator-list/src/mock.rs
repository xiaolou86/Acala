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

//! Mocks for homa validator list module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, Balance, CurrencyId, TokenSymbol};
use sp_core::H256;
use sp_runtime::{traits::IdentityLookup, BuildStorage};
use sp_std::cell::RefCell;
use std::collections::HashMap;
use support::ExchangeRate;

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const VALIDATOR_1: AccountId = 2;
pub const VALIDATOR_2: AccountId = 3;
pub const VALIDATOR_3: AccountId = 4;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

mod homa_validator_list {
	pub use super::super::*;
}

impl frame_system::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type RuntimeCall = RuntimeCall;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = Everything;
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ConstU32<100>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
	type RuntimeHoldReason = ();
	type FreezeIdentifier = ();
	type MaxHolds = ();
	type MaxFreezes = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
}

pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;
pub type LDOTCurrency = orml_currencies::Currency<Runtime, GetLiquidCurrencyId>;

impl orml_currencies::Config for Runtime {
	type MultiCurrency = OrmlTokens;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

thread_local! {
	pub static SHARES: RefCell<HashMap<(AccountId, AccountId), Balance>> = RefCell::new(HashMap::new());
	pub static ACCUMULATED_SLASH: RefCell<Balance> = RefCell::new(0);
}

pub struct MockOnSlash;
impl Happened<Balance> for MockOnSlash {
	fn happened(amount: &Balance) {
		ACCUMULATED_SLASH.with(|v| *v.borrow_mut() += amount);
	}
}

pub struct MockOnIncreaseGuarantee;
impl Happened<(AccountId, AccountId, Balance)> for MockOnIncreaseGuarantee {
	fn happened(info: &(AccountId, AccountId, Balance)) {
		let (account_id, relaychain_id, amount) = info;
		SHARES.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(share) = old_map.get_mut(&(*account_id, *relaychain_id)) {
				*share = share.saturating_add(*amount);
			} else {
				old_map.insert((*account_id, *relaychain_id), *amount);
			};

			*v.borrow_mut() = old_map;
		});
	}
}

pub struct MockOnDecreaseGuarantee;
impl Happened<(AccountId, AccountId, Balance)> for MockOnDecreaseGuarantee {
	fn happened(info: &(AccountId, AccountId, Balance)) {
		let (account_id, relaychain_id, amount) = info;
		SHARES.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(share) = old_map.get_mut(&(*account_id, *relaychain_id)) {
				*share = share.saturating_sub(*amount);
			} else {
				old_map.insert((*account_id, *relaychain_id), Default::default());
			};

			*v.borrow_mut() = old_map;
		});
	}
}

pub struct MockLiquidStakingExchangeProvider;
impl ExchangeRateProvider for MockLiquidStakingExchangeProvider {
	fn get_exchange_rate() -> ExchangeRate {
		ExchangeRate::saturating_from_rational(1, 2)
	}
}

parameter_types! {
	pub static MockBlockNumberProvider: u64 = 0;
}

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u64;

	fn current_block_number() -> Self::BlockNumber {
		Self::get()
	}
}

ord_parameter_types! {
	pub const Admin: AccountId = 10;
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RelaychainAccountId = AccountId;
	type LiquidTokenCurrency = LDOTCurrency;
	type MinBondAmount = ConstU128<100>;
	type BondingDuration = ConstU64<100>;
	type ValidatorInsuranceThreshold = ConstU128<200>;
	type FreezeOrigin = EnsureSignedBy<Admin, AccountId>;
	type SlashOrigin = EnsureSignedBy<Admin, AccountId>;
	type OnSlash = MockOnSlash;
	type LiquidStakingExchangeRateProvider = MockLiquidStakingExchangeProvider;
	type WeightInfo = ();
	type OnIncreaseGuarantee = MockOnIncreaseGuarantee;
	type OnDecreaseGuarantee = MockOnDecreaseGuarantee;
	type BlockNumberProvider = MockBlockNumberProvider;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		OrmlTokens: orml_tokens,
		PalletBalances: pallet_balances,
		OrmlCurrencies: orml_currencies,
		HomaValidatorListModule: homa_validator_list,
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![(ALICE, LDOT, 1000), (BOB, LDOT, 1000)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
