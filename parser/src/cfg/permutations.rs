//! Implements Heap's algorithm.

/*
From Wikipedia:

procedure generate(k: integer, A : array of any):
    if k = 1 then
        output(A)
    else
        // Generate permutations with k-th unaltered
        // Initially k = length(A)
        generate(k - 1, A)

        // Generate permutations for k-th swapped with each k-1 initial
        for i := 0; i < k-1; i += 1 do
            // Swap choice dependent on parity of k (even or odd)
            if k is even then
                swap(A[i], A[k-1]) // zero-indexed, the k-th is at k-1
            else
                swap(A[0], A[k-1])
            end if
            generate(k - 1, A)
        end for
    end if
*/

/// Heap's algorithm
pub fn gen_permutations<T: Clone + Default>(a: &[T]) -> Vec<Vec<T>> {
    let mut a2 = vec![Default::default(); a.len()];
    a2.clone_from_slice(a);
    let mut outs = vec![];
    heaps_alg(a.len(), &mut a2, &mut outs);
    outs
}

fn heaps_alg<T: Clone>(k: usize, a: &mut [T], outs: &mut Vec<Vec<T>>) {
    if k == 1 {
        outs.push(a.to_vec());
    } else {
        heaps_alg(k - 1, a, outs);
        for i in 0..k - 1 {
            if (k % 2) == 0 {
                a.swap(i, k - 1);
            } else {
                a.swap(0, k - 1);
            }
            heaps_alg(k - 1, a, outs);
        }
    }
}
