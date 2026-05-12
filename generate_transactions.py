import csv
import random
import sys

# Generate a large number of transactions to get roudh idea of the performance 
# don't pretend to be realistic on the tx distribution, just generate in a way
# it is easy to verify the correctness of the final balances.


random.seed(42)
num_users = 20000
user_ids = random.sample(range(1, 65536), num_users)

# store deposit tx ids per user
user_deposits = {uid: [] for uid in user_ids}

writer = csv.writer(sys.stdout)
writer.writerow(['type', 'client', 'tx', 'amount'])
tx_id = 1
# deposit rounds, interleave users
for round_num in range(50):
    for client in user_ids:
        amount = client * 0.0002
        writer.writerow(['deposit', client, tx_id, f'{amount:.4f}'])
        user_deposits[client].append(tx_id)
        tx_id += 1
# withdrawal rounds, interleave users
for round_num in range(5):
    for client in user_ids:
        amount = client * 0.0001
        writer.writerow(['withdrawal', client, tx_id, f'{amount:.4f}'])
        tx_id += 1
# final disputes/resolutions per user
for client in user_ids:
    deposits = user_deposits[client]
    if client % 2 == 0:
        disputed_tx = deposits[0]
        writer.writerow(['dispute', client, disputed_tx, ''])
        writer.writerow(['resolve', client, disputed_tx, ''])
    else:
        disputed_tx_1 = deposits[0]
        disputed_tx_2 = deposits[1]
        writer.writerow(['dispute', client, disputed_tx_1, ''])
        writer.writerow(['dispute', client, disputed_tx_2, ''])
        writer.writerow(['chargeback', client, disputed_tx_1, ''])
