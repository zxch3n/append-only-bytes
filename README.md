<div align="center">
  <h1><code>append-only-bytes</code></h2>
  <h3><a href="https://docs.rs/append-only-bytes">Documentation</a></h3>
  <p></p>
</div>

If an array is append-only and guarantees that all the existing data is immutable, 
then we may share the slices of this array safely across threads while the owner 
can still safely append new data to it. 

It's safe because no mutable byte has more than one owner. 

When there is not enough capacity for new append, `AppendOnlyBytes` will not 
deallocate the old memory if there is `ByteSlice` referring to it.

# Example

```rust
let mut bytes = AppendOnlyBytes::new();
bytes.push_slice(&[1, 2, 3]);
let slice: BytesSlice = bytes.slice(1..);
bytes.push_slice(&[4, 5, 6]);
assert_eq!(&*slice, &[2, 3]);
assert_eq!(bytes.as_bytes(), &[1, 2, 3, 4, 5, 6])
```
