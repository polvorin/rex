# REX - A Toy Payment Engine


# General Design

An Engine process transactions (_Deposit|Withdrawl|Dispute|Resolve|Chargeback_) and keep track of user balances. Transactions from a given user must be feed in order.

We store the balance' amounts as integers, as we are interested in 4 positions past the decimal point, we multiply by 10000 and round.  12.3456  is stored as 123456. Easier to handle, avoid precission errors.

Engine works over _Transaction_ type.  The actual CSV input is translated into this
before being feed into the engine. (If there were other formats than CSV, we would transalte them to the _Transaction_ representation).

A transaction refers to a single balance, and that can be identified by the client_id (present in every transaction),  we use this fact to "shard" the user space and have multiple Engine threads,  one per shard.  The main loop dispatch the transaction to the correct shard based on the client_id.



# Assumptions:

* The toy engine only accept disputes for _Deposit_ transactions (not _Withdrawls_).
That's what I interpret from the wording on the requirements:

>The transaction shouldn't be reversed yet but the associated funds should be held. This means hat the clients available funds should decrease by the amount disputed, their held funds should increase by the amount disputed, while their total funds should remain the same." 

That makes sense for disputing a _Deposit_ (a portion of a user' funds are locked until the dispute is resolved),  but for _Withdrawals_ that would mean the available funds will increase, making it possible to withdraw the funds again  *while the dispute wasn't resolved yet*.
 
* Once an account is locked, all further transactions are rejected
> If a chargeback occurs the client's account should be immediately frozen."

I assummed this mean that no deposit, withdrawl, dispute, resolution nor chargeback would be acepted for the account once it get locked.  If we wanted to still allow dispute/resolution/chargebacks,  it would be an easy change. 


# Correctness

There are unit tests covering the transactions semantics (with the assumptions explained in previous section),  and simple CSV files to run against.

For keeping track of account'  funds on holds and available funds,  I choose to not keep them separately,  rather,  compute it based on the Account' total balance and disputed transactions.  This way we have a single source of truth for the available balance, and avoid having to update multiple fields that could lead to inconsistencies if not done carefully.  Also, transactions are _moved_ from undisputed to disputed sets, making the code evident.

There is no way to inform of rejected transactions, so we just ignore them.
(there is warn logging done to stderr with these rejected transactions,  you can check by running with RUST_LOG=warn)



# Performance

The work of the Engine is simple and entirely CPU bound, doesn't require any IO nor blocking operation. For this each engine runs on his own dedicated thread, Async / Tokio won't give us any benefit here.

Given the little processing we need to do per transaction, performance will _very_ likely be dictated by:

* input/output (how fast we can read and parse the data, how fast we can serialize and write).
* syncronization overhead, due the data transfer from main loop to the Engine threads

I prefer to keep the multiple engine approach as the syncronization overhead would be minor as soon as the transactions do any meaningful work.  It would be easy to
reduce the overhead as well by batching (buffer and pass several transactions to the engine instead of one-by-one)

If instead of reading from stdin we were receiving and processing multiple streams in parallel, as from concurrent TCP streams, the existing design remains appropiate, as each of these streams would just route the transaction to the appropiate Engine as done here (how to handle and read all those TCP connections, is something we _could_ use Tokio for).

The fact that client_id are u16, would have allowed us to put balances on a contigous array and reference them directly rather than using a HashMap.  But prefered the HashMap for simplicity and because any gain here is meaningful only if transactions remains this simple, and still likely minor compared to the other cost described.



# Memory Usage

We keep all balances in memory.
While we process transactions in a streamming way as readed from stdin, in order
to process disputes/chargebacks we also need to keep information about past _Deposits_ around.


# Quick Profiling

A quick profiling using [samply](https://github.com/mstange/samply) confirms the intuition regarding the performance characteristic of the engine.


* Actual transaction processing is fast, the engines themselves are underutilized waiting for more data to be feed.  
* syncronization cost is high

!(flamegraph of main loop thread)[main_loop_thread.png]


* Inside each engine' thread,  engine is blocked waiting for input rather than saturate the CPU


!(flamegraph of engine loop thread)[engine_thread.png]


The single, simplest thing to improve performance of this toy project would be to send transaction in batches to the engines instead of one-by-one.