//===----------------------------------------------------------------------===//
//
// Adapted from the Swift.org open source project (stdlib/public/core/Diffing.swift,
// tag swift-6.3.3-RELEASE, commit 064859e41d68596f486c5d724401cb370f260409,
// SHA-256 0bee4637d036f97d22c29bd6a4f0c73b598bedc95f17978cddce5158025da285).
//
// Copyright (c) 2015 - 2019 Apple Inc. and the Swift project authors
// Licensed under Apache License v2.0 with Runtime Library Exception
//
// See https://swift.org/LICENSE.txt for license information
// See https://swift.org/CONTRIBUTORS.txt for the list of Swift project authors
//
// Modified 2026-10-11 for Bello Box: safe arrays instead of unsafe buffers, a
// plain struct for the k-vectors, and offsets returned as two edit masks instead
// of a CollectionDifference. The search order, overlap choices, region order and
// prefix/suffix shrinking are unchanged, so ambiguous alignments match 6.3.3.
// The full license is in ThirdPartyNotices.txt.
//
//===----------------------------------------------------------------------===//

/// The linear-space Myers difference from Swift 6.3.3's standard library.
///
/// `difference(from:)` belongs to the OS runtime, so the rows it chose for
/// equally short edit scripts changed with the user's macOS: the 6.0 runtime
/// kept the blank line of `"a\n\nb\n"` → `"a\nb\n\n"` unchanged where 6.3.3
/// marks it added. Shipping the algorithm makes Compare the same everywhere.
enum MyersDifference {
    /// Offsets in `old` that are removed and offsets in `new` that are inserted.
    static func changes<T>(from old: [T], to new: [T], by equal: (T, T) -> Bool) -> (removed: [Bool], inserted: [Bool]) {
        var removed = [Bool](repeating: false, count: old.count)
        var inserted = [Bool](repeating: false, count: new.count)
        let box = Rect(left: 0, top: 0, right: old.count, bottom: new.count).shrunk(old, new, equal)
        var search = Search(size: box.size)
        var stack: [Rect] = []
        var current = box
        while true {
            let (edit, snake) = search.middleSnake(in: current, old, new, equal)
            switch edit {
            case .remove(let offset): removed[offset] = true
            case .insert(let offset): inserted[offset] = true
            case nil: break
            }
            guard let snake else {
                guard let next = stack.popLast() else { return (removed, inserted) }
                current = next
                continue
            }
            let head = current.cropped(toTopLeftOf: snake).shrunk(old, new, equal)
            let tail = current.cropped(toBottomRightOf: snake).shrunk(old, new, equal)
            current = head
            stack.append(tail)
        }
    }

    private enum Edit { case remove(Int), insert(Int) }

    /// A region of the edit graph: `left..<right` in old, `top..<bottom` in new.
    private struct Rect {
        var left: Int, top: Int, right: Int, bottom: Int
        var width: Int { right - left }
        var height: Int { bottom - top }
        var size: Int { width + height }
        var delta: Int { width - height }
        var isOdd: Bool { !delta.isMultiple(of: 2) }
        var max: Int { (size + 1) / 2 }

        func cropped(toTopLeftOf limit: Rect) -> Rect { Rect(left: left, top: top, right: limit.left, bottom: limit.top) }
        func cropped(toBottomRightOf limit: Rect) -> Rect { Rect(left: limit.right, top: limit.bottom, right: right, bottom: bottom) }

        func shrunk<T>(_ old: [T], _ new: [T], _ equal: (T, T) -> Bool) -> Rect {
            var r = self
            while r.left < r.right, r.top < r.bottom, equal(old[r.left], new[r.top]) { r.left += 1; r.top += 1 }
            while r.right > r.left, r.bottom > r.top, equal(old[r.right - 1], new[r.bottom - 1]) { r.right -= 1; r.bottom -= 1 }
            return r
        }
    }

    /// The forward and backward k-vectors, each indexed `-size...size`.
    private struct Search {
        var forward: [Int]
        var backward: [Int]
        let offset: Int

        init(size: Int) {
            // One spare slot on each side: depth 0 writes k = 1 even when size is 0.
            offset = size + 1
            forward = [Int](repeating: 0, count: size * 2 + 3)
            backward = forward
        }

        subscript(forward k: Int) -> Int {
            get { forward[offset + k] }
            set { forward[offset + k] = newValue }
        }

        subscript(backward c: Int) -> Int {
            get { backward[offset + c] }
            set { backward[offset + c] = newValue }
        }

        mutating func middleSnake<T>(in box: Rect, _ old: [T], _ new: [T], _ equal: (T, T) -> Bool) -> (Edit?, Rect?) {
            guard box.size > 1 else {
                if box.size == 0 { return (nil, nil) }
                return box.width == 1 ? (.remove(box.left), nil) : (.insert(box.top), nil)
            }
            self[forward: 1] = box.left
            self[backward: 1] = box.bottom
            for depth in 0...box.max {
                if box.isOdd {
                    if let result = forwardSearch(in: box, depth: depth, old, new, equal) { return result }
                    _ = backwardSearch(in: box, depth: depth, old, new, equal)
                } else {
                    _ = forwardSearch(in: box, depth: depth, old, new, equal)
                    if let result = backwardSearch(in: box, depth: depth, old, new, equal) { return result }
                }
            }
            preconditionFailure("A nonempty edit graph always has a middle snake")
        }

        private mutating func forwardSearch<T>(in box: Rect, depth: Int, _ old: [T], _ new: [T], _ equal: (T, T) -> Bool) -> (Edit, Rect)? {
            for k in stride(from: depth, through: -depth, by: -2) {
                var newRight: Int, newBottom: Int
                let newLeft: Int, newTop: Int
                let edit: Edit
                if k == -depth || (k != depth && self[forward: k - 1] < self[forward: k + 1]) {
                    newRight = self[forward: k + 1]
                    newLeft = newRight
                    newBottom = box.top + (newRight - box.left) - k
                    edit = .insert(newBottom - 1)
                } else {
                    newLeft = self[forward: k - 1]
                    newRight = newLeft + 1
                    newBottom = box.top + (newRight - box.left) - k
                    edit = .remove(newLeft)
                }
                newTop = depth == 0 || newRight != newLeft ? newBottom : newBottom - 1
                while newRight < box.right && newBottom < box.bottom, equal(old[newRight], new[newBottom]) {
                    newRight += 1
                    newBottom += 1
                }
                self[forward: k] = newRight
                // Only odd-sized boxes check for overlap when moving forward.
                guard box.isOdd else { continue }
                let c = k - box.delta
                if c >= -(depth - 1) && c <= depth - 1 && newBottom >= self[backward: c] {
                    return (edit, Rect(left: newLeft, top: newTop, right: newRight, bottom: newBottom))
                }
            }
            return nil
        }

        private mutating func backwardSearch<T>(in box: Rect, depth: Int, _ old: [T], _ new: [T], _ equal: (T, T) -> Bool) -> (Edit, Rect)? {
            for c in stride(from: depth, through: -depth, by: -2) {
                let k = c + box.delta
                var newLeft: Int, newTop: Int
                let newRight: Int, newBottom: Int
                let edit: Edit
                if c == -depth || (c != depth && self[backward: c - 1] > self[backward: c + 1]) {
                    newTop = self[backward: c + 1]
                    newBottom = newTop
                    newLeft = box.left + (newTop - box.top) + k
                    edit = .remove(newLeft)
                } else {
                    newBottom = self[backward: c - 1]
                    newTop = newBottom - 1
                    newLeft = box.left + (newTop - box.top) + k
                    edit = .insert(newTop)
                }
                newRight = depth == 0 || newTop != newBottom ? newLeft : newLeft + 1
                while newLeft > box.left && newTop > box.top, equal(old[newLeft - 1], new[newTop - 1]) {
                    newLeft -= 1
                    newTop -= 1
                }
                self[backward: c] = newTop
                // Only even-sized boxes check for overlap when moving backward.
                guard !box.isOdd else { continue }
                if k >= -depth && k <= depth && newLeft <= self[forward: k] {
                    return (edit, Rect(left: newLeft, top: newTop, right: newRight, bottom: newBottom))
                }
            }
            return nil
        }
    }
}
