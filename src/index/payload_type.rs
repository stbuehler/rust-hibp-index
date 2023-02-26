mod seal_trait {
	pub trait U8Array: Default + AsRef<[u8]> + AsMut<[u8]> {
		const SIZE: usize;
	}

	impl<const N: usize> U8Array for [u8; N]
	where
		[u8; N]: Default,
	{
		const SIZE: usize = N;
	}
}

/// A type carrying payload data; must be readable and writable as raw u8 array
///
/// Types implementing this should usually just be a wrapper around some (fixed length)
/// u8 array to help properly interpreting the result.
///
// Would be nicer if we could have:
// pub trait PayloadData: Default + AsRef<[u8; Self::Size]> + AsMut<[u8; Self::Size]> { const SIZE: usize; }
pub trait PayloadData:
	Default + Clone + AsRef<Self::PayloadArray> + AsMut<Self::PayloadArray>
{
	type PayloadArray: seal_trait::U8Array;
}

/// Make internal size accessible
pub trait PayloadDataExt: PayloadData {
	const SIZE: usize = <Self::PayloadArray as seal_trait::U8Array>::SIZE;

	/// Data of `Self::SIZE` length, but type system can't handle it yet
	fn data(&self) -> &[u8] {
		self.as_ref().as_ref()
	}

	/// Mutable data of `Self::SIZE` length, but type system can't handle it yet
	fn data_mut(&mut self) -> &mut [u8] {
		self.as_mut().as_mut()
	}
}

impl<P: PayloadData> PayloadDataExt for P {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct NoPayload;

impl AsRef<[u8; 0]> for NoPayload {
	fn as_ref(&self) -> &[u8; 0] {
		&[]
	}
}

impl AsMut<[u8; 0]> for NoPayload {
	fn as_mut(&mut self) -> &mut [u8; 0] {
		&mut []
	}
}

impl PayloadData for NoPayload {
	type PayloadArray = [u8; 0];
}
