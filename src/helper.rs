use heapless::Vec;

/// Calculates the mediana by first sorting the [`Vec`],
/// and calculates it based on an even or odd number of records.
pub fn find_mediana<const N: usize>(measurements: &mut Vec<u16, N>) -> u16 {
    match measurements.len() {
        0 => return 0,
        1 => return measurements[0],
        // continue
        _ => {}
    }

    measurements.sort_unstable_by(Ord::cmp);

    // if even number of records
    let mediana = if measurements.len() % 2 == 0 {
        let second_index = measurements.len() / 2;
        let first_index = second_index - 1;

        // will floor the result!
        (measurements[first_index] + measurements[second_index]) / 2
    } else {
        // if odd
        // calculate the mediana, this floors the index!
        let mediana_index = measurements.len() / 2;

        measurements[mediana_index]
    };

    mediana
}


#[cfg(test)]
mod test {
    use supper::find_mediana;

    #[test]
    fn test_find_mediana() {
        const MEASUREMENTS: usize = 15;

        // odd number of measurements
        {
            let measurements = [78_u16, 99, 25, 1, 46];

            let mut mediana_records =
                Vec::<u16, MEASUREMENTS>::from_slice(&measurements).unwrap();
            let mediana = find_mediana(&mut mediana_records);

            // assert expected sorting
            {
                let expected_sorted = [1_u16, 25, 46, 78, 9];
                assert_eq!(mediana_records, expected_sorted);
            }

            // assert expected mediana index
            {
                let mediana_index = mediana_records
                    .iter()
                    .enumerate()
                    .find_map(|(index, value)| if &mediana == value { Some(index) } else { None })
                    .expect("Should find the mediana index as the value is part of vector");

                assert_eq!(2, mediana_index)
            }

            assert_eq!(mediana, 46);
        }

        // even number of measurements
        {
            // 1, 15, 18, 39
            let measurements = [18_u16, 39, 15, 1];

            let mut mediana_records =
                Vec::<u16, MEASUREMENTS>::from_slice(&measurements).unwrap();
            let mediana = find_mediana(&mut mediana_records);

            assert_eq!(mediana, 11);
        }
    }
}