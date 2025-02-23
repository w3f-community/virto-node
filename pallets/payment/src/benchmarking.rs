use super::*;

use crate::{Pallet as Payment, Payment as PaymentStore};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use sp_runtime::Percent;
use sp_std::vec;
use virto_primitives::Asset;

const SEED: u32 = 0;
const INITIAL_AMOUNT: u128 = 100;
const SOME_AMOUNT: u128 = 80;
const MAX_REMARK_LENGTH: u8 = 50;

fn get_currency_id() -> Asset {
	Asset::default()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
	let events = frame_system::Pallet::<T>::events();
	let system_event: <T as frame_system::Config>::Event = generic_event.into();
	// compare to the last event record
	let frame_system::EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

benchmarks! {
	where_clause { where T::Asset: MultiCurrency<
		<T as frame_system::Config>::AccountId,
		CurrencyId = Asset, Balance = u128
	>
}

	// create a new payment with remark sucessfully
	pay {
		let caller = whitelisted_caller();
		let _ = T::Asset::deposit(get_currency_id(), &caller, INITIAL_AMOUNT);
		let recipent = account("recipient", 0, SEED);
		let x in 1..MAX_REMARK_LENGTH.into();
		let remark : BoundedDataOf<T> = vec![u8::MAX; x.try_into().unwrap()].try_into().unwrap();
	}: _(RawOrigin::Signed(caller.clone()), recipent, get_currency_id(), SOME_AMOUNT, Some(remark.clone()))
	verify {
		assert_last_event::<T>(Event::<T>::PaymentCreated { from: caller, asset: get_currency_id(), amount: SOME_AMOUNT, remark: Some(remark)}.into());
	}

	// release an existing payment succesfully
	release {
		let caller = whitelisted_caller();
		let _ = T::Asset::deposit(get_currency_id(), &caller, INITIAL_AMOUNT);
		let recipent : T::AccountId = account("recipient", 0, SEED);
		Payment::<T>::pay(RawOrigin::Signed(caller.clone()).into(), recipent.clone(), get_currency_id(), SOME_AMOUNT, None)?;
	}: _(RawOrigin::Signed(caller.clone()), recipent.clone())
	verify {
		assert_last_event::<T>(Event::<T>::PaymentReleased { from: caller, to: recipent}.into());
	}

	// cancel an existing payment succesfully
	cancel {
		let caller = whitelisted_caller();
		let _ = T::Asset::deposit(get_currency_id(), &caller, INITIAL_AMOUNT);
		let recipent : T::AccountId = account("recipient", 0, SEED);
		Payment::<T>::pay(RawOrigin::Signed(caller.clone()).into(), recipent.clone(), get_currency_id(), SOME_AMOUNT, None)?;
	}: _(RawOrigin::Signed(recipent.clone()), caller.clone())
	verify {
		assert_last_event::<T>(Event::<T>::PaymentCancelled { from: caller, to: recipent}.into());
	}

	// resolve an existing payment to cancellation - this is the most complex path
	resolve_payment {
		let caller = whitelisted_caller();
		let _ = T::Asset::deposit(get_currency_id(), &caller, INITIAL_AMOUNT);
		let recipent : T::AccountId = account("recipient", 0, SEED);
		Payment::<T>::pay(RawOrigin::Signed(caller.clone()).into(), recipent.clone(), get_currency_id(), SOME_AMOUNT, None)?;
		let resolver = PaymentStore::<T>::get(caller.clone(), recipent.clone()).unwrap().resolver_account;
	}: _(RawOrigin::Signed(resolver), caller.clone(), recipent.clone(), Percent::from_percent(100))
	verify {
		assert_last_event::<T>(Event::<T>::PaymentResolved { from: caller, to: recipent, recipient_share: Percent::from_percent(100)}.into());
	}

	// creator of payment creates a refund request
	request_refund {
		let caller = whitelisted_caller();
		let _ = T::Asset::deposit(get_currency_id(), &caller, INITIAL_AMOUNT);
		let recipent : T::AccountId = account("recipient", 0, SEED);
		Payment::<T>::pay(RawOrigin::Signed(caller.clone()).into(), recipent.clone(), get_currency_id(), SOME_AMOUNT, None)?;
	}: _(RawOrigin::Signed(caller.clone()), recipent.clone())
	verify {
		assert_last_event::<T>(Event::<T>::PaymentCreatorRequestedRefund { from: caller, to: recipent, expiry: 601u32.into() }.into());
	}

	// recipient of a payment can dispute a refund request
	dispute_refund {
		let caller = whitelisted_caller();
		let _ = T::Asset::deposit(get_currency_id(), &caller, INITIAL_AMOUNT);
		let recipent : T::AccountId = account("recipient", 0, SEED);
		Payment::<T>::pay(RawOrigin::Signed(caller.clone()).into(), recipent.clone(), get_currency_id(), SOME_AMOUNT, None)?;
		Payment::<T>::request_refund(RawOrigin::Signed(caller.clone()).into(), recipent.clone())?;
	}: _(RawOrigin::Signed(recipent.clone()), caller.clone())
	verify {
		assert_last_event::<T>(Event::<T>::PaymentRefundDisputed { from: caller, to: recipent}.into());
	}

	// recipient of a payment can create a payment reqest
	request_payment {
		let caller : T::AccountId = whitelisted_caller();
		let sender : T::AccountId = account("recipient", 0, SEED);
	}: _(RawOrigin::Signed(caller.clone()), sender.clone(), get_currency_id(), SOME_AMOUNT)
	verify {
		assert_last_event::<T>(Event::<T>::PaymentRequestCreated { from: sender, to: caller}.into());
	}

	// payment request can be completed by the sender
	accept_and_pay {
		let sender : T::AccountId = whitelisted_caller();
		let receiver : T::AccountId = account("recipient", 0, SEED);
		let _ = T::Asset::deposit(get_currency_id(), &sender, INITIAL_AMOUNT);
		Payment::<T>::request_payment(RawOrigin::Signed(receiver.clone()).into(), sender.clone(), get_currency_id(), SOME_AMOUNT)?;
	}: _(RawOrigin::Signed(sender.clone()), receiver.clone())
	verify {
		assert_last_event::<T>(Event::<T>::PaymentRequestCompleted { from: sender, to: receiver}.into());
	}

	// the weight to remove a scheduled task
	remove_task {
		let sender : T::AccountId = whitelisted_caller();
		let receiver : T::AccountId = account("recipient", 0, SEED);
		let mut task_list : ScheduledTaskList<T> = Default::default();
		task_list.try_insert((sender.clone(), receiver.clone()), ScheduledTask { task: Task::Cancel, when: 1u32.into() }).unwrap();
		ScheduledTasks::<T>::set(task_list);
	}: {
		ScheduledTasks::<T>::mutate(|task_list| {
			task_list.remove(&(sender.clone(), receiver.clone()))
		});
	} verify {
		assert!(ScheduledTasks::<T>::get().is_empty());
	}

}

impl_benchmark_test_suite!(Payment, crate::mock::new_test_ext(), crate::mock::Test,);
