# Contributing to lowfi

[[version française](./docs/fr/CONTRIBUER.md)]

There are a few guidelines outlined here that will make it more likely for your PR to be accepted.
Only ones that are less obvious are going to be listed. If you need to ask, it's probably a no.

## 1. AI

You can use AI for searching or if there's something minor and tedious (eg. tests) that you'd like
to avoid having to do manually.

With that said, if it is noticeable that you used AI then it is way too much.
AI generated PR's do not help maintainers, it's just a hassle and frequently wastes their time.

## 2. Smaller is better

Try and make it so that each PR is one contained feature. Adding multiple features in a PR is usually a bad idea.
This is also so that individual features can be approved or denied, rather than that having to be for a more significant
chunk of code.

## 3. Keep lowfi simple

lowfi is supposed simple program. For now, no changes to the initial user-facing UI will be accepted.
The UI of lowfi playing a song has stayed identical since the first versions, since complicating it
detracts from it's purpose.

More complex features, like fancy colors or cover art, will not be accepted ever. Implementations of
acceptable features should also be simple and not too obtrusive. Even if a feature is simple,
if it is very complex to implement, then it won't be accepted.
