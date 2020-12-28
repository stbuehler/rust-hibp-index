# HIBP indexer

Goal: fast and easy lookup in ["Have I Been Pwned" Passwords][hibp-password] local database.

## Prepare

Download (torrent or direct) `pwned-passwords-sha1-ordered-by-hash-v7.7z` from [hibp-password], then extract `pwned-passwords-sha1-ordered-by-hash-v7.txt`:

    7z x pwned-passwords-sha1-ordered-by-hash-v7.7z

This will take a while.

Then build the index `hibp-sha1.index`:

    cargo run --release --bin hibp-create-sha1-index

On my system I get the following file sizes:

    10789612 hibp-sha1.index
    11526460 pwned-passwords-sha1-ordered-by-hash-v7.7z
    26404944 pwned-passwords-sha1-ordered-by-hash-v7.txt

So the index is even smaller than the compressed download!

## Use

Start the lookup process:

    cargo run --release --bin hibp-lookup

And then enter passwords or SHA1-hashes on stdin (you could also send input via pipe); it will then tell you whether the SHA1 hash is contained in the database or not.

## How it works

- The SHA1 hashes are sorted into buckets
  - The bucket index is a fixed-length (bitstring) prefix of the hash
  - Right now 20 bits are used; i.e. 2^20 buckets (about 1 million), with about 600 entries per bucket; the first 20 bit of the hash are used as bucket index.
- In each bucket only the bytes that are not (fully) part of the bucket index are stored for each entry
  - In the current confiuration this means 2 bytes less to store per entry
- For each bucket store the file offset where the bucket starts; also store where the last bucket ends
  - This index is stored at the end of the file and compressed with `DEFLATE`; it is loaded into memory when opening the index.
- To search for an entry simply lookup the bucket start and end (the start of the following bucket!) file offsets
  - Theoretically could do binary or even interpolation search as the entries are sorted; right now using a linear search (with early abort).
  - More complicated search would require advanced [`BufRead`] implementation (the [default rust one][`BufReader`] resets the buffer on every seek), and linear seems fast enough.

The file format also allows for a fixed size "payload" ("value") per entry; not used right now.

Given the hashes should be distributed evenly there is no reason to implement "optionally" nested indices.

The selection of the index size ("20 bits") is more an optimization to compact common prefixes across many entries and to get away with linear search.
If the database grows a lot more the index size can be increased to 24 bits (the implementation rejects larger indices, as memory usage will grow to store it: 24 bits already requires 128M); afterwards binary / interpolation search needs to be implemented.

## File size

The original text file uses hexadecimal representation of the SHA1 hashes; the 7z compression will mostly undo that (i.e. use half the number of bytes to store the hash binary); it should also be able to compress shared prefixes for sequential entries.
The original text file also includes a "prevalence" number after the entry.

The created index probably can't be compressed much - within a bucket the entries might still have some "structure" in the first few bits in each entry (as they are sorted and should be distributed quite evenly), but then high entropy should follow - after all, compressing a cryptographic hash should be difficult :)

## Future work

The NTLM-hashes probably can be stored in a similar fashion.

[hibp-password]: https://haveibeenpwned.com/Passwords
[`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
[`BufReader`]: https://doc.rust-lang.org/std/io/struct.BufReader.html
