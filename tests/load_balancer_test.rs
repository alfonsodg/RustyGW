use rustway::features::load_balancer::{LoadBalanceStrategy, LoadBalancer};

#[test]
fn test_round_robin_distribution() {
    let lb = LoadBalancer::new();
    let results: Vec<usize> = (0..6).map(|_| lb.next_index(3, &LoadBalanceStrategy::RoundRobin)).collect();
    assert_eq!(results, vec![0, 1, 2, 0, 1, 2]);
}

#[test]
fn test_round_robin_two_backends() {
    let lb = LoadBalancer::new();
    let results: Vec<usize> = (0..4).map(|_| lb.next_index(2, &LoadBalanceStrategy::RoundRobin)).collect();
    assert_eq!(results, vec![0, 1, 0, 1]);
}

#[test]
fn test_round_robin_single_backend() {
    let lb = LoadBalancer::new();
    let results: Vec<usize> = (0..5).map(|_| lb.next_index(1, &LoadBalanceStrategy::RoundRobin)).collect();
    assert_eq!(results, vec![0, 0, 0, 0, 0]);
}

#[test]
fn test_random_strategy_in_bounds() {
    let lb = LoadBalancer::new();
    for _ in 0..100 {
        let idx = lb.next_index(3, &LoadBalanceStrategy::Random);
        assert!(idx < 3);
    }
}

#[test]
fn test_load_balancer_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let lb = Arc::new(LoadBalancer::new());
    let mut handles = vec![];

    for _ in 0..4 {
        let lb = lb.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                let idx = lb.next_index(3, &LoadBalanceStrategy::RoundRobin);
                assert!(idx < 3);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
