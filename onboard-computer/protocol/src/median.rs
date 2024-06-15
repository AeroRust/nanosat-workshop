use heapless::Vec;

pub fn calculate_median<const N: usize>(records: &mut Vec<u16, N>) -> u16 {
    if records.is_empty() {
        return 0;
    } else if records.len() == 1 {
        return records[0];
    }

    records.sort_unstable_by(Ord::cmp);

    // if even number of records
    if records.len() % 2 == 0 {
        let second_index = records.len() / 2;
        let first_index = second_index - 1;

        // will floor the result!
        (records[first_index] + records[second_index]) / 2
    } else {
        // if odd
        // calculate the mediana, this floors the index!
        let median_index = records.len() / 2;

        records[median_index]
    }
}