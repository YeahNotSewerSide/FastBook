use fastbook::*;
use rand::prelude::*;
use std::thread;
use std::time::Duration;

fn reader<T: Ord + Clone + Copy + std::fmt::Debug>(order_book: OrderBookRead<T>) {
    for _ in 0..1000 {
        let best_bid = match order_book.best_bid() {
            Ok(v) => v,
            Err(_) => {
                println!("Deallocated!");
                return;
            }
        };
        println!(
            "Best bid: {:?} | Best ask: {:?}",
            best_bid,
            order_book.best_ask()
        );
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn test() {
    let (mut book_write, book_read) = orderbook::<i64>();

    let writer_thread = thread::spawn(move || {
        let mut rng = rand::thread_rng();

        for _ in 0..1000 {
            let mut values: Vec<(i64, f64)> = Vec::with_capacity(1000);

            for _ in 0..1000 {
                values.push((rng.gen::<u16>() as i64, rng.gen()));
            }
            book_write.update_bids(&values);
            book_write.update_asks(&values);
        }
    });

    let book_copy = book_read.clone();
    let reader_thread1 = thread::spawn(|| reader(book_copy));
    let book_copy = book_read.clone();
    let _reader_thread2 = thread::spawn(|| reader(book_copy));

    writer_thread.join().unwrap();
    reader_thread1.join().unwrap();
}
