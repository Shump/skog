use std::boxed::Box;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ForestEdge {
    Trailing,
    Leading,
}

impl From<bool> for ForestEdge {
    fn from(b: bool) -> Self {
        match b {
            false => ForestEdge::Trailing,
            true => ForestEdge::Leading,
        }
    }
}

pub fn pivot(e: ForestEdge) -> ForestEdge {
    match e {
        ForestEdge::Trailing => ForestEdge::Leading,
        ForestEdge::Leading => ForestEdge::Trailing,
    }
}

pub fn is_leading(e: ForestEdge) -> bool {
    e == ForestEdge::Leading
}

pub fn is_trailing(e: ForestEdge) -> bool {
    e == ForestEdge::Trailing
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum NextPrior {
    Prior,
    Next,
}

struct NodeBase<D> {
    trailing_prior: *mut D,
    trailing_next: *mut D,
    leading_prior: *mut D,
    leading_next: *mut D,
}

impl<D> NodeBase<D> {
    fn new() -> Self {
        Self {
            trailing_prior: std::ptr::null_mut(),
            trailing_next: std::ptr::null_mut(),
            leading_prior: std::ptr::null_mut(),
            leading_next: std::ptr::null_mut(),
        }
    }

    fn init(&mut self, node: *mut D) {
        self.trailing_prior = node;
        self.trailing_next = node;
        self.leading_prior = node;
        self.leading_next = node;
    }

    fn link_mut(&mut self, edge: ForestEdge, link: NextPrior) -> &mut *mut D {
        use ForestEdge::*;
        use NextPrior::*;
        match (edge, link) {
            (Trailing, Prior) => &mut self.trailing_prior,
            (Trailing, Next) => &mut self.trailing_next,
            (Leading, Prior) => &mut self.leading_prior,
            (Leading, Next) => &mut self.leading_next,
        }
    }

    fn link(&self, edge: ForestEdge, link: NextPrior) -> *mut D {
        use ForestEdge::*;
        use NextPrior::*;
        match (edge, link) {
            (Trailing, Prior) => self.trailing_prior,
            (Trailing, Next) => self.trailing_next,
            (Leading, Prior) => self.leading_prior,
            (Leading, Next) => self.leading_next,
        }
    }
}

struct Node<T> {
    base: NodeBase<Node<T>>,
    data: MaybeUninit<T>,
    _phantom: PhantomData<T>,
}

impl<T> Node<T> {
    fn uninit() -> Self {
        Self {
            base: NodeBase::new(),
            data: MaybeUninit::uninit(),
            _phantom: PhantomData,
        }
    }

    fn new(data: T) -> Self {
        Self {
            base: NodeBase::new(),
            data: MaybeUninit::new(data),
            _phantom: PhantomData,
        }
    }
}

trait CursorLike {
    type Item;
    fn move_next(&mut self);
    fn move_prev(&mut self);
    fn current(&self) -> Option<Self::Item>;
}

struct CursorIterator<T: CursorLike> {
    cursor: T,
}

impl<T: CursorLike> Iterator for CursorIterator<T> {
    type Item = <T as CursorLike>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.cursor.current();
        self.cursor.move_next();
        item
    }
}

struct RawCursor<T> {
    node: *mut Node<T>,
    edge: ForestEdge,
}

impl<T> Clone for RawCursor<T> {
    fn clone(&self) -> Self {
        RawCursor { ..*self }
    }
}

impl<T> Copy for RawCursor<T> {}

impl<T> PartialEq for RawCursor<T> {
    fn eq(&self, other: &Self) -> bool {
        self.equal(other)
    }
}

impl<T> RawCursor<T> {
    fn new(node: *mut Node<T>, edge: ForestEdge) -> Self {
        RawCursor {
            node,
            edge,
        }
    }

    fn pivot(&mut self) {
        self.edge = pivot(self.edge);
    }

    fn leading_of(&self) -> Self {
        RawCursor { edge: ForestEdge::Leading, ..*self }
    }

    fn trailing_of(&self) -> Self {
        RawCursor { edge: ForestEdge::Trailing, ..*self }
    }

    fn is_leading(&self) -> bool {
        is_leading(self.edge)
    }

    #[allow(dead_code)]
    fn is_trailing(&self) -> bool {
        is_trailing(self.edge)
    }

    fn equal_node(&self, y: &Self) -> bool {
        self.node == y.node
    }

    unsafe fn has_children(&self) -> bool {
        !self.equal_node(&self.leading_of().next())
    }

    unsafe fn move_next(&mut self) {
        let next = (*self.node).base.link(self.edge, NextPrior::Next);
        if is_leading(self.edge) {
            self.edge = (next != self.node).into();
        } else {
            let link = (*next).base.link(ForestEdge::Leading, NextPrior::Prior);
            let edge = (link == self.node).into();
            self.edge = edge;
        }
        self.node = next;
    }

    unsafe fn move_prev(&mut self) {
        let next = (*self.node).base.link(self.edge, NextPrior::Prior);
        if is_leading(self.edge) {
            let link = (*next).base.link(ForestEdge::Trailing, NextPrior::Next);
            self.edge = (link != self.node).into();
        } else {
            self.edge = (next == self.node).into();
        }
        self.node = next;
    }

    unsafe fn move_next_child(&mut self) {
        self.pivot();
        self.move_next();
    }

    unsafe fn move_prev_child(&mut self) {
        self.move_prev();
        self.pivot();
    }

    unsafe fn current<'a>(&self) -> Option<&'a T> {
        Some((*self.node).data.assume_init_ref())
    }

    unsafe fn current_mut<'a>(&mut self) -> Option<&'a mut T> {
        Some((*self.node).data.assume_init_mut())
    }

    unsafe fn insert(&self, item: T) -> Self {
        let node = Box::into_raw(Box::new(Node::new(item)));
        (*node).base.init(node);
        let result = RawCursor {
            node,
            edge: ForestEdge::Leading,
        };
        set_next(&self.prev(), &result);
        set_next(&result.next(), &self);
        result
    }

    unsafe fn erase_range(self, last: RawCursor<T>) -> Self {
        let first = self;

        let mut stack_depth = 0usize;
        let mut position = first;

        while position != last {
            if position.edge == ForestEdge::Leading {
                stack_depth += 1;
                position.move_next();
            } else {
                if stack_depth > 0 {
                    position = position.erase();
                } else {
                    position.move_next();
                }
                stack_depth = std::cmp::max(0, stack_depth - 1);
            }
        }
        last
    }

    unsafe fn erase(self) -> Self {
        /*
            https://github.com/stlab/libraries/blob/c86c645eb6696360b49a2ff05aa25aa07f5b94d2/stlab/forest.hpp
            NOTE (sparent) : After the first call to set_next() the invariants of the forest are
            violated and we can't determing leading/trailing if we navigate from the affected node.
            So we gather all the iterators up front then do the set_next calls.
        */

        let leading_prior = self.leading_of().prev();
        let leading_next = self.leading_of().next();
        let trailing_prior = self.trailing_of().prev();
        let trailing_next = self.trailing_of().next();

        if self.has_children() {
            set_next(&leading_prior, &leading_next);
            set_next(&trailing_prior, &trailing_next);
        } else {
            set_next(&leading_prior, &trailing_next);
        }

        {
            std::ptr::drop_in_place((*self.node).data.as_mut_ptr());
            Box::from_raw(self.node);
        }

        if self.is_leading() {
            leading_prior.next()
        } else {
            trailing_next
        }
    }

    unsafe fn splice(&mut self, first: RawCursor<T>, last: RawCursor<T>) -> RawCursor<T> {
        if first == last || &first == self { // XXX don't need?
            return *self;
        }

        let back = last.prev();

        set_next(&first.prev_child(), &last);

        set_next(&self.prev(), &first);
        set_next(&back, self);

        first
    }

    unsafe fn next(&self) -> Self {
        let mut clone = RawCursor { ..*self };
        clone.move_next();
        clone
    }

    #[allow(dead_code)]
    unsafe fn next_child(&self) -> Self {
        let mut clone = RawCursor { ..*self };
        clone.move_next_child();
        clone = clone.leading_of(); // new child iterators are leading
        clone
    }

    unsafe fn prev(&self) -> Self {
        let mut clone = RawCursor { ..*self };
        clone.move_prev();
        clone
    }

    unsafe fn prev_child(&self) -> Self {
        let mut clone = RawCursor { ..*self };
        clone.move_prev_child();
        clone = clone.leading_of(); // new child iterators are leading
        clone
    }

    fn equal(&self, y: &RawCursor<T>) -> bool {
        self.node == y.node && self.edge == y.edge
    }
}

impl<T> std::fmt::Debug for RawCursor<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        if self.edge == ForestEdge::Leading {
            write!(f, "-->{:?}", self.node)
        } else {
            write!(f, "{:?}-->", self.node)
        }
    }
}

unsafe fn set_next<T>(x: &RawCursor<T>, y: &RawCursor<T>) {
    *(*x.node).base.link_mut(x.edge, NextPrior::Next) = y.node;
    *(*y.node).base.link_mut(y.edge, NextPrior::Prior) = x.node;
}

struct EdgeCursor<'a, T: 'a> {
    edge: ForestEdge,
    cursor: Cursor<'a, T>,
}

impl<'a, T> EdgeCursor<'a, T> {
    fn new(edge: ForestEdge, cursor: Cursor<'a, T>) -> Self {
        Self { edge, cursor }
    }

    fn into_iter(self) -> CursorIterator<Self> {
        CursorIterator { cursor: self }
    }
}

impl<'a, T> CursorLike for EdgeCursor<'a, T> {
    type Item = &'a T;

    fn move_next(&mut self) {
        self.cursor.move_next();
        self.cursor.find_edge(self.edge);
    }

    fn move_prev(&mut self) {
        self.cursor.move_prev();
        self.cursor.find_edge_reverse(self.edge);
    }

    fn current(&self) -> Option<Self::Item> {
        self.cursor.current()
    }
}

pub struct Cursor<'a, T: 'a> {
    forest: &'a Forest<T>,
    cursor: RawCursor<T>,
}

impl<'a, T> PartialEq for Cursor<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.cursor == other.cursor
    }
}

impl<'a, T> Eq for Cursor<'a, T> {}

impl<'a, T> Cursor<'a, T> {
    pub fn leading_of(&mut self) {
        self.cursor = self.cursor.leading_of();
    }

    pub fn trailing_of(&mut self) {
        self.cursor = self.cursor.trailing_of();
    }

    pub fn move_next(&mut self) {
        unsafe { self.cursor.move_next(); }
    }

    pub fn move_prev(&mut self) {
        unsafe { self.cursor.move_prev(); }
    }

    pub fn edge(&self) -> ForestEdge {
        self.cursor.edge
    }

    pub fn current(&self) -> Option<&'a T> {
        unsafe {
            if self.cursor.equal_node(&self.forest.unsafe_root()) {
                None
            } else {
                self.cursor.current()
            }
        }
    }

    fn find_edge(&mut self, edge: ForestEdge) {
        while self.cursor.edge != edge {
            self.move_next();
        }
    }

    fn find_edge_reverse(&mut self, edge: ForestEdge) {
        while self.cursor.edge != edge {
            self.move_prev();
        }
    }
}

impl<'a, T> std::fmt::Debug for Cursor<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.cursor)
    }
}

pub struct CursorMut<'a, T: 'a> {
    forest: &'a mut Forest<T>,
    cursor: RawCursor<T>,
}

impl<'a, T> CursorMut<'a, T> {
    pub fn size(&mut self) -> usize {
        self.forest.size()
    }

    pub fn empty(&self) -> bool {
        self.forest.empty()
    }

    pub fn leading_of(&mut self) {
        self.cursor = self.cursor.leading_of();
    }

    pub fn trailing_of(&mut self) {
        self.cursor = self.cursor.trailing_of();
    }

    pub fn move_next(&mut self) {
        unsafe { self.cursor.move_next(); }
    }

    pub fn move_prev(&mut self) {
        unsafe { self.cursor.move_prev(); }
    }

    pub fn edge(&self) -> ForestEdge {
        self.cursor.edge
    }

    pub fn current(&mut self) -> Option<&'a mut T> {
        unsafe {
            if self.cursor.equal_node(&self.forest.unsafe_root()) {
                None
            } else {
                self.cursor.current_mut()
            }
        }
    }

    pub fn insert(&mut self, item: T) {
        if self.forest.size_valid() {
            self.forest.size += 1;
        }
        unsafe { self.cursor.insert(item); }
    }

    pub fn insert_and_move(&mut self, item: T) {
        if self.forest.size_valid() {
            self.forest.size += 1;
        }
        self.cursor = unsafe { self.cursor.insert(item) };
    }

    pub fn splice(&mut self, mut x: Forest<T>) {
        if self.forest.size_valid() && x.size_valid() {
            self.forest.size += x.size();
        } else {
            self.forest.size = 0;
        }
        unsafe { self.cursor.splice(x.unsafe_begin(), x.unsafe_end()); }
    }

    pub fn splice_and_move(&mut self, mut x: Forest<T>) {
        if self.forest.size_valid() && x.size_valid() {
            self.forest.size += x.size();
        } else {
            self.forest.size = 0;
        }
        self.cursor = unsafe { self.cursor.splice(x.unsafe_begin(), x.unsafe_end()) };
    }

    #[allow(dead_code)]
    fn remove(&mut self) {
        if self.forest.size_valid() {
            self.forest.size -= 1;
        }
        self.cursor = unsafe { self.cursor.erase() };
    }
}

impl<'a, T> std::fmt::Debug for CursorMut<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.cursor)
    }
}

pub struct Forest<T> {
    size: usize,
    tail: *mut Node<T>,
}

impl<T> Forest<T> {
    pub fn new() -> Self {
        unsafe {
            let this = Self {
                size: 0,
                tail: Box::into_raw(Box::new(Node::uninit())),
            };
            (*this.tail).base.init(this.tail);
            this
        }
    }

    pub fn size(&mut self) -> usize {
        if !self.size_valid() {
            let c = EdgeCursor::new(ForestEdge::Leading, self.begin());
            let i = c.into_iter();
            self.size = i.count();
        }
        self.size
    }

    pub fn size_valid(&self) -> bool {
        self.size != 0 || self.empty()
    }

    pub fn empty(&self) -> bool {
        self.begin() == self.end()
    }

    pub fn root(&self) -> Cursor<T> {
        Cursor { forest: self, cursor: self.unsafe_root() }
    }

    pub fn root_mut(&mut self) -> CursorMut<T> {
        let cursor = self.unsafe_root();
        CursorMut { forest: self, cursor }
    }

    pub fn begin(&self) -> Cursor<T> {
        Cursor { forest: self, cursor: self.unsafe_begin() }
    }

    pub fn begin_mut(&mut self) -> CursorMut<T> {
        let mut c = self.root_mut();
        c.move_next();
        c
    }

    pub fn end(&self) -> Cursor<T> {
        Cursor { forest: self, cursor: self.unsafe_end() }
    }

    pub fn end_mut(&mut self) -> CursorMut<T> {
        let cursor = self.unsafe_end();
        CursorMut { forest: self, cursor }
    }

    pub fn clear(&mut self) {
        let begin = self.unsafe_begin();
        let end = self.unsafe_end();
        unsafe { begin.erase_range(end); }
        self.size = 0;
    }

    fn unsafe_root(&self) -> RawCursor<T> {
        RawCursor {
            node: self.tail_mut(),
            edge: ForestEdge::Leading,
        }
    }

    fn unsafe_begin(&self) -> RawCursor<T> {
        unsafe {
            let mut c = self.unsafe_root();
            c.move_next();
            c
        }
    }

    fn unsafe_end(&self) -> RawCursor<T> {
        RawCursor::new(self.tail_mut(), ForestEdge::Trailing)
    }

    fn tail_mut(&self) -> *mut Node<T> {
        self.tail
    }
}

impl<T> Drop for Forest<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn print(f: &Forest<(char, std::rc::Rc<()>)>) {
        struct Tabs(usize);

        impl std::fmt::Display for Tabs {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
                for _ in 0..self.0 {
                    write!(f, "\t")?;
                }
                Ok(())
            }
        }

        let mut cur = f.begin();
        let mut depth = 0;
        while cur != f.end() {
            use ForestEdge::*;
            match (cur.edge(), cur.current().unwrap().0) {
                (Leading, value) => {
                    println!("{}<{}>", Tabs(depth), value);
                    depth += 1;
                }
                (Trailing, value) => {
                    depth -= 1;
                    println!("{}</{}>", Tabs(depth), value);
                }
            }
            cur.move_next();
        }
    }

    #[test]
    fn big_test_forest() {
        let mut data = std::rc::Rc::new(());

        let mut c = Forest::new();
        let mut cur = c.end_mut();
        cur.insert_and_move(('C', data.clone()));
        cur.trailing_of();

        cur.insert(('F', data.clone()));
        cur.insert(('G', data.clone()));
        cur.insert(('H', data.clone()));

        let mut d = Forest::new();
        let mut cur = d.end_mut();
        cur.insert_and_move(('D', data.clone()));
        cur.trailing_of();

        cur.insert(('I', data.clone()));
        cur.insert(('J', data.clone()));
        cur.insert(('K', data.clone()));

        let mut e = Forest::new();
        let mut cur = e.end_mut();
        cur.insert_and_move(('E', data.clone()));

        let mut b = Forest::new();
        let mut cur = b.end_mut();
        cur.insert_and_move(('B', data.clone()));
        cur.trailing_of();

        cur.splice(c);
        cur.splice(d);
        cur.splice(e);

        let mut a = Forest::new();
        let mut cur = a.end_mut();
        cur.insert_and_move(('A', data.clone()));
        cur.trailing_of();

        cur.splice(b);

        print(&a);

        assert_eq!(a.size(), 11);

        let mut cur = a.begin();
        assert_eq!(cur.current().map(|(c, _)| c), Some(&'A'));

        cur.move_next();
        /**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'B'));

        cur.move_next();
        /**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'C'));

        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'F'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'F'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'G'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'G'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'H'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'H'));

        cur.move_next();
        /**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'C'));

        cur.move_next();
        /**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'D'));

        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'I'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'I'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'J'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'J'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'K'));
        cur.move_next();
        /**//**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'K'));

        cur.move_next();
        /**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'D'));

        cur.move_next();
        /**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'E'));
        cur.move_next();
        /**//**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'E'));

        cur.move_next();
        /**/assert_eq!(cur.current().map(|(c, _)| c), Some(&'B'));

        cur.move_next();
        assert_eq!(cur.current().map(|(c, _)| c), Some(&'A'));

        cur.move_next();
        assert_eq!(cur, a.end());

        a.clear();
        assert!(a.empty());

        assert!(std::rc::Rc::get_mut(&mut data).is_some());
    }
}
