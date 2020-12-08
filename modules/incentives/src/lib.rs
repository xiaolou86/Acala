#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{EnsureOrigin, Get, Happened},
	transactional,
	weights::Weight,
	IterableStorageMap,
};
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, RewardHandler};
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, UniqueSaturatedInto, Zero},
	DispatchResult, FixedPointNumber, ModuleId, RuntimeDebug,
};
use sp_std::prelude::*;
use support::{CDPTreasury, DEXIncentives, DEXManager, EmergencyShutdown, Rate};

mod default_weight;
mod mock;
mod tests;

pub trait WeightInfo {
	fn deposit_dex_share() -> Weight;
	fn withdraw_dex_share() -> Weight;
	fn claim_rewards() -> Weight;
	fn update_loans_incentive_rewards(c: u32) -> Weight;
	fn update_dex_incentive_rewards(c: u32) -> Weight;
	fn update_homa_incentive_reward() -> Weight;
	fn update_dex_saving_rates(c: u32) -> Weight;
}

/// PoolId for various rewards pools
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum PoolId {
	/// Rewards(ACA) pool for users who open CDP
	Loans(CurrencyId),
	/// Rewards(ACA) pool for market makers who provide dex liquidity
	DexIncentive(CurrencyId),
	/// Rewards(AUSD) pool for liquidators who provide dex liquidity to
	/// participate automatic liquidation
	DexSaving(CurrencyId),
	/// Rewards(ACA) pool for users who staking by Homa protocol
	Homa,
}

decl_error! {
	/// Error for incentives module.
	pub enum Error for Module<T: Config> {
		/// Share amount is not enough
		NotEnough,
		/// Invalid currency id
		InvalidCurrencyId,
	}
}

decl_event!(
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Deposit DEX share. \[who, dex_share_type, deposit_amount\]
		DepositDEXShare(AccountId, CurrencyId, Balance),
		/// Withdraw DEX share. \[who, dex_share_type, withdraw_amount\]
		WithdrawDEXShare(AccountId, CurrencyId, Balance),
	}
);

pub trait Config:
	frame_system::Config + orml_rewards::Config<Share = Balance, Balance = Balance, PoolId = PoolId>
{
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// The vault account to keep rewards for type LoansIncentive PoolId
	type LoansIncentivePool: Get<Self::AccountId>;

	/// The vault account to keep rewards for type DexIncentive and DexSaving
	/// PoolId
	type DexIncentivePool: Get<Self::AccountId>;

	/// The vault account to keep rewards for type HomaIncentive PoolId
	type HomaIncentivePool: Get<Self::AccountId>;

	/// The period to accumulate rewards
	type AccumulatePeriod: Get<Self::BlockNumber>;

	/// The incentive reward type (should be ACA)
	type IncentiveCurrencyId: Get<CurrencyId>;

	/// The saving reward type (should be AUSD)
	type SavingCurrencyId: Get<CurrencyId>;

	/// The origin which may update incentive related params
	type UpdateOrigin: EnsureOrigin<Self::Origin>;

	/// CDP treasury to issue rewards in AUSD
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// Currency for transfer/issue assets
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// DEX to supply liquidity info
	type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

	/// Emergency shutdown.
	type EmergencyShutdown: EmergencyShutdown;

	/// The module id, keep DEXShare LP.
	type ModuleId: Get<ModuleId>;

	/// Weight information for the extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_storage! {
	trait Store for Module<T: Config> as Incentives {
		/// Mapping from collateral currency type to its loans incentive reward amount per period
		pub LoansIncentiveRewards get(fn loans_incentive_rewards): map hasher(twox_64_concat) CurrencyId => Balance;

		/// Mapping from dex liquidity currency type to its loans incentive reward amount per period
		pub DEXIncentiveRewards get(fn dex_incentive_rewards): map hasher(twox_64_concat) CurrencyId => Balance;

		/// Homa incentive reward amount
		pub HomaIncentiveReward get(fn homa_incentive_reward): Balance;

		/// Mapping from dex liquidity currency type to its saving rate
		pub DEXSavingRates get(fn dex_saving_rates): map hasher(twox_64_concat) CurrencyId => Rate;
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// The vault account to keep rewards for type LoansIncentive PoolId
		const LoansIncentivePool: T::AccountId = T::LoansIncentivePool::get();

		/// The vault account to keep rewards for type DexIncentive and DexSaving PoolId
		const DexIncentivePool: T::AccountId = T::DexIncentivePool::get();

		/// The vault account to keep rewards for type HomaIncentive PoolId
		const HomaIncentivePool: T::AccountId = T::HomaIncentivePool::get();

		/// The period to accumulate rewards
		const AccumulatePeriod: T::BlockNumber = T::AccumulatePeriod::get();

		/// The incentive reward type (should be ACA)
		const IncentiveCurrencyId: CurrencyId = T::IncentiveCurrencyId::get();

		/// The saving reward type (should be AUSD)
		const SavingCurrencyId: CurrencyId = T::SavingCurrencyId::get();

		#[weight = <T as Config>::WeightInfo::deposit_dex_share()]
		#[transactional]
		pub fn deposit_dex_share(origin, lp_currency_id: CurrencyId, amount: Balance) {
			let who = ensure_signed(origin)?;
			Self::do_deposit_dex_share(&who, lp_currency_id, amount)?;
		}

		#[weight = <T as Config>::WeightInfo::withdraw_dex_share()]
		#[transactional]
		pub fn withdraw_dex_share(origin, lp_currency_id: CurrencyId, amount: Balance) {
			let who = ensure_signed(origin)?;
			Self::do_withdraw_dex_share(&who, lp_currency_id, amount)?;
		}

		#[weight = <T as Config>::WeightInfo::claim_rewards()]
		#[transactional]
		pub fn claim_rewards(origin, pool_id: T::PoolId) {
			let who = ensure_signed(origin)?;
			<orml_rewards::Module<T>>::claim_rewards(&who, pool_id);
		}

		#[weight = <T as Config>::WeightInfo::update_loans_incentive_rewards(updates.len() as u32)]
		#[transactional]
		pub fn update_loans_incentive_rewards(
			origin,
			updates: Vec<(CurrencyId, Balance)>,
		) {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (currency_id, amount) in updates {
				LoansIncentiveRewards::insert(currency_id, amount);
			}
		}

		#[weight = <T as Config>::WeightInfo::update_dex_incentive_rewards(updates.len() as u32)]
		#[transactional]
		pub fn update_dex_incentive_rewards(
			origin,
			updates: Vec<(CurrencyId, Balance)>,
		) {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (currency_id, amount) in updates {
				ensure!(currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);
				DEXIncentiveRewards::insert(currency_id, amount);
			}
		}

		#[weight = <T as Config>::WeightInfo::update_homa_incentive_reward()]
		#[transactional]
		pub fn update_homa_incentive_reward(
			origin,
			update: Balance,
		) {
			T::UpdateOrigin::ensure_origin(origin)?;
			HomaIncentiveReward::put(update);
		}

		#[weight = <T as Config>::WeightInfo::update_dex_saving_rates(updates.len() as u32)]
		#[transactional]
		pub fn update_dex_saving_rates(
			origin,
			updates: Vec<(CurrencyId, Rate)>,
		) {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (currency_id, rate) in updates {
				ensure!(currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);
				DEXSavingRates::insert(currency_id, rate);
			}
		}
	}
}

impl<T: Config> DEXIncentives<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn do_deposit_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		ensure!(lp_currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);

		T::Currency::transfer(lp_currency_id, who, &Self::account_id(), amount)?;
		<orml_rewards::Module<T>>::add_share(
			who,
			PoolId::DexIncentive(lp_currency_id),
			amount.unique_saturated_into(),
		);
		<orml_rewards::Module<T>>::add_share(who, PoolId::DexSaving(lp_currency_id), amount.unique_saturated_into());

		Self::deposit_event(RawEvent::DepositDEXShare(who.clone(), lp_currency_id, amount));
		Ok(())
	}

	fn do_withdraw_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		ensure!(lp_currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);
		ensure!(
			<orml_rewards::Module<T>>::share_and_withdrawn_reward(PoolId::DexIncentive(lp_currency_id), &who).0
				>= amount && <orml_rewards::Module<T>>::share_and_withdrawn_reward(PoolId::DexSaving(lp_currency_id), &who)
				.0 >= amount,
			Error::<T>::NotEnough,
		);

		T::Currency::transfer(lp_currency_id, &Self::account_id(), &who, amount)?;
		<orml_rewards::Module<T>>::remove_share(
			who,
			PoolId::DexIncentive(lp_currency_id),
			amount.unique_saturated_into(),
		);
		<orml_rewards::Module<T>>::remove_share(who, PoolId::DexSaving(lp_currency_id), amount.unique_saturated_into());

		Self::deposit_event(RawEvent::WithdrawDEXShare(who.clone(), lp_currency_id, amount));
		Ok(())
	}
}

impl<T: Config> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}
}

pub struct OnUpdateLoan<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, CurrencyId, Amount, Balance)> for OnUpdateLoan<T> {
	fn happened(info: &(T::AccountId, CurrencyId, Amount, Balance)) {
		let (who, currency_id, adjustment, previous_amount) = info;
		let adjustment_abs =
			sp_std::convert::TryInto::<Balance>::try_into(adjustment.saturating_abs()).unwrap_or_default();

		if !adjustment_abs.is_zero() {
			let new_share_amount = if adjustment.is_positive() {
				previous_amount.saturating_add(adjustment_abs)
			} else {
				previous_amount.saturating_sub(adjustment_abs)
			};

			<orml_rewards::Module<T>>::set_share(who, PoolId::Loans(*currency_id), new_share_amount);
		}
	}
}

impl<T: Config> RewardHandler<T::AccountId, T::BlockNumber> for Module<T> {
	type Share = Balance;
	type Balance = Balance;
	type PoolId = PoolId;
	type CurrencyId = CurrencyId;

	fn accumulate_reward(now: T::BlockNumber, mut callback: impl FnMut(PoolId, Balance)) -> Vec<(CurrencyId, Balance)> {
		let mut accumulated_rewards: Vec<(CurrencyId, Balance)> = vec![];

		if !T::EmergencyShutdown::is_shutdown() && now % T::AccumulatePeriod::get() == Zero::zero() {
			let mut accumulated_incentive: Balance = Zero::zero();
			let mut accumulated_saving: Balance = Zero::zero();
			let incentive_currency_id = T::IncentiveCurrencyId::get();
			let saving_currency_id = T::SavingCurrencyId::get();

			for (pool_id, pool_info) in orml_rewards::Pools::<T>::iter() {
				if !pool_info.total_shares.is_zero() {
					match pool_id {
						PoolId::Loans(currency_id) => {
							let incentive_reward = Self::loans_incentive_rewards(currency_id);

							// TODO: transfer from RESERVED TREASURY instead of issuing
							if !incentive_reward.is_zero()
								&& T::Currency::deposit(
									incentive_currency_id,
									&T::LoansIncentivePool::get(),
									incentive_reward,
								)
								.is_ok()
							{
								callback(pool_id, incentive_reward);
								accumulated_incentive = accumulated_incentive.saturating_add(incentive_reward);
							}
						}

						PoolId::DexIncentive(currency_id) => {
							let incentive_reward = Self::dex_incentive_rewards(currency_id);

							// TODO: transfer from RESERVED TREASURY instead of issuing
							if !incentive_reward.is_zero()
								&& T::Currency::deposit(
									incentive_currency_id,
									&T::DexIncentivePool::get(),
									incentive_reward,
								)
								.is_ok()
							{
								callback(pool_id, incentive_reward);
								accumulated_incentive = accumulated_incentive.saturating_add(incentive_reward);
							}
						}

						PoolId::DexSaving(currency_id) => {
							let dex_saving_rate = Self::dex_saving_rates(currency_id);
							if !dex_saving_rate.is_zero() {
								if let Some((currency_id_a, currency_id_b)) = currency_id.split_dex_share_currency_id()
								{
									// accumulate saving reward only for liquidity pool of saving currency id
									let saving_currency_amount = if currency_id_a == saving_currency_id {
										T::DEX::get_liquidity_pool(saving_currency_id, currency_id_b).0
									} else if currency_id_b == saving_currency_id {
										T::DEX::get_liquidity_pool(saving_currency_id, currency_id_a).0
									} else {
										Zero::zero()
									};

									if !saving_currency_amount.is_zero() {
										let saving_reward = dex_saving_rate.saturating_mul_int(saving_currency_amount);
										if T::CDPTreasury::issue_debit(
											&T::DexIncentivePool::get(),
											saving_reward,
											false,
										)
										.is_ok()
										{
											callback(pool_id, saving_reward);
											accumulated_saving = accumulated_saving.saturating_add(saving_reward);
										}
									}
								}
							}
						}

						PoolId::Homa => {
							let incentive_reward = Self::homa_incentive_reward();

							// TODO: transfer from RESERVED TREASURY instead of issuing
							if !incentive_reward.is_zero()
								&& T::Currency::deposit(
									incentive_currency_id,
									&T::HomaIncentivePool::get(),
									incentive_reward,
								)
								.is_ok()
							{
								callback(pool_id, incentive_reward);
								accumulated_incentive = accumulated_incentive.saturating_add(incentive_reward);
							}
						}
					}
				}
			}

			if !accumulated_incentive.is_zero() {
				accumulated_rewards.push((incentive_currency_id, accumulated_incentive));
			}
			if !accumulated_saving.is_zero() {
				accumulated_rewards.push((saving_currency_id, accumulated_saving));
			}
		}

		accumulated_rewards
	}

	fn payout(who: &T::AccountId, pool_id: PoolId, amount: Balance) {
		let (pool_account, currency_id) = match pool_id {
			PoolId::Loans(_) => (T::LoansIncentivePool::get(), T::IncentiveCurrencyId::get()),
			PoolId::DexIncentive(_) => (T::DexIncentivePool::get(), T::IncentiveCurrencyId::get()),
			PoolId::DexSaving(_) => (T::DexIncentivePool::get(), T::SavingCurrencyId::get()),
			PoolId::Homa => (T::HomaIncentivePool::get(), T::IncentiveCurrencyId::get()),
		};

		// payout the reward to user from the pool. it should not affect the
		// process, ignore the result to continue. if it fails, just the user will not
		// be rewarded, there will not increase user balance.
		let _ = T::Currency::transfer(currency_id, &pool_account, &who, amount);
	}
}
