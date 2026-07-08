pub(crate) fn split_fields(data: &[u8], num_targets: usize) -> Vec<Vec<u8>> {
    if num_targets == 1 {
        return vec![data.to_vec()];
    }

    let mut fields = Vec::new();
    let mut start = 0;
    for (i, &b) in data.iter().enumerate() {
        if b == b' ' || b == b'\t' {
            if i > start
                && let Some(slice) = data.get(start..i)
            {
                fields.push(slice.to_vec());
            }
            start = i + 1;
        }
    }
    if let Some(slice) = data.get(start..) {
        fields.push(slice.to_vec());
    }

    if fields.len() == num_targets {
        return fields;
    }

    if fields.len() < num_targets {
        for _ in 0..(num_targets - fields.len()) {
            fields.push(Vec::new());
        }
        fields
    } else {
        let mut result: Vec<Vec<u8>> = Vec::with_capacity(num_targets);
        for field in fields.iter().take(num_targets - 1) {
            result.push(field.clone());
        }
        let mut last = Vec::new();
        for (i, field) in fields.iter().enumerate().skip(num_targets - 1) {
            if i > num_targets - 1 {
                last.push(b' ');
            }
            last.extend_from_slice(field);
        }
        result.push(last);
        result
    }
}
