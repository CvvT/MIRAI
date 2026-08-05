// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// A test that visits `BlockVisitor::visit_unevaluated_const` with a non-promoted MIR constant
// that resolves to an instance with generic args.

pub trait MyTrait {
    const MY_ASSOC_CONST: u8;
}

pub struct MyConstGenericImpl<const N: u8>;

impl<const N: u8> MyTrait for MyConstGenericImpl<N> {
    const MY_ASSOC_CONST: u8 = N;
}

pub fn foo<T>(_x: T) {}

pub fn compare_unmatched<const N: usize>(x: usize) -> bool {
    x >= N
}

pub struct ConstGenericWrapper<const N: usize>(pub [u8; N]);

impl<const N: usize> std::ops::Deref for ConstGenericWrapper<N> {
    type Target = [u8; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn deref_unmatched<const N: usize>(value: &ConstGenericWrapper<N>) -> &[u8; N] {
    std::ops::Deref::deref(value)
}

pub fn main() {
    foo(MyConstGenericImpl::<1>::MY_ASSOC_CONST);
}
