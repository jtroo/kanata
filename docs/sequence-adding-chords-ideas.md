# Sequence improvement: sequence chords

## Preface

This document is a record of designing/braindumping
for the improvement to the sequences feature to add chord support.
It is left in an informal and disorganized state 
— as opposed to a being presentable design doc — out of laziness.
Apologies ahead of time if you read this and it's hard to follow,
feel free to contribute a PR to create a new and more polished doc.

## Motivation

The desire is to be able to add output chords to a sequence.
The effect of this is that: `(S-(a b))` can be differentiated from `(S-a b)`.
Today, chords are not supported at all.
The two sequences above could be written as `(lsft a b)`;
however, the code today has no way to decide the difference
between `lsft` being applied to only `a` or to both `a` and `b`.

The feature will codenamed "seqchords" for brevity in this document.

## An exploration of an idea: track releases?

Today, the sequence `(lsft a b c)` doesn't care when the `lsft`,
or even `a` or `b` are released relative to when the subsequent keys
are pressed. However, with seqchords, the code could be potentially changed to
make sequences release-aware.

It seems a little difficult to integrate
this into the trie structure used to track sequences though.
With an implementation that is release-aware,
it seems like the code would need to figure out how to conditionally
add release events to the trie, depending if the seq was
`(lsft a)` or `(S-a)`

For now, I think a different approach would be better.

## A different idea: modify presses held with mod keys.

The current sequence type is `Vec<u16>` since keys don't fit into `u8`.
However, there are fewer than 1024 (2^10) keys total.
That means there are 6 bits to play with.
6 bits are enough for the types of modifiers (of which there are 4),
but differentiating both sides (increases to 8).
Perhaps one only cares to use both left and right shifts though, and maybe
both left and right alts.
One could also use a `u32` instead, but that seems unnecessary for now.
I see no backwards compatibility issues if one
desired that change in the future.

With this in mind, while modifiers are held, set the upper unused bits of
the stored `u16` values.

### Backwards compatibility?

This does mean that `(lsft a b)` behaves differently
with vs. without seqchords.
Unless maybe the code automatically generates the various permutations
of this type of sequence, but that seems complicated.
Or maybe have a `u16` with a special bit pattern that could be used
to differentiate between `(S-(a b))` and `(lsft a b)`.
For now, let's say that the bit pattern is `0xFFFF`.
If a modifier is pressed and the sequence `[..., <mod>, 0xFFFF]`
exists in the trie: continue processing the sequence in mod-aware mode.

OR for simplicity, just say "screw backwards compatibility" and force users
to be clear about what they mean and define the extra permutations, if they
want them. I prefer this.

### Data format examples

Let's begin the description of the new data format.
Since shifted keys seem like they will be the main use case for seqchords,
only that will be described in this document for now.
Here are the numerical key values relevant to the examples.

- `a:    0x001E`
- `b:    0x0030`.
- `lsft: 0x002A`.

This differs by OS, but that's not important.

The transformation of `(lsft a b)` to a sequence in the trie today
looks like:

- `[0x002A, 0x001E, 0x0030]`

This will remain unchanged with seqchords.
Let's say that chorded keys using `lsft`
will have the otherwise-unused MSB (bit 15) set.

The transformation of some sequences using chords will be:

1. `(S-(a b)) => [0x802A, 0x801E, 0x8030]`
2. `  (S-a b) => [0x802A, 0x801E, 0x0030]`
3. `(S-a S-b) => [0x802A, 0x801E, 0x802A, 0x8030]`

Notably, `lsft` is modifying its own upper bits.
This should simplify the implementation logic
so that the code does not need to add a special-case check
that the newly-pressed key is itself a modifier.

One may need to define different sequences if one wishes to use both
left and right shifts to be able to trigger these shifted sequences.
The syntax does not exist today, but maybe `(S-(a b))` and `(RS-(a b))`
as an example for left and right shifts.
The reason different sequences would be required is because the
sequence->trie check operates on the integers that correspond to the keycodes.

Consideration: maybe there could be transformations for the right modifier
keys to ensure they get translated to the left modifier keys.
This seems like it could be a sensible default to start with.
If a change is desired in the future to **not** do this transformation,
it doesn't seem too difficult to add a configuration item to do so.
For now that will be left out, deferring to the YAGNI principle.

### Backwards compatibility revisited

Thinking back on the topic of backwards compatibility,
I'm scrapping that idea of special bit patterns.
I thought of a probably-better way:
backtracking with modifier cancellation.

By default when seqchords gets added,
the modified bit patterns will be used
to check in the trie for valid sequences.
However, with a `defcfg` item `sequence-backtrack-modcancel`
— which should be `yes` by default for back-compat reasons —
if the code encounters an invalid sequence with the modded bit pattern,
it will try again with the unmodded bit pattern, and only if that does not
match will sequence-mode end with an invalid termination.
This backtracking can be turned off if desired,
e.g. if it behaves badly in some future seqchords use cases.
