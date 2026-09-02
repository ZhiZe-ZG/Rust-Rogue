# Why This Project Uses a Maintenance-Linked License

## Background

This project began as a reimplementation of the classic **Rogue: Exploring the Dungeons of Doom** in Rust.

The original Rogue source code is preserved under its original license. New code, redesigns, refactoring, documentation, and other original contributions made for this project are separately licensed under the project's Maintenance-Linked Commercial License.

The purpose of the latter license is not to prevent people from studying, modifying, sharing, or experimenting with the project.

Its purpose is to establish a different relationship between **commercial use** and **long-term maintenance**.

## The Basic Idea

Software can remain commercially valuable for a very long time after its original development has stopped.

A company may integrate a project into a commercial product once, make relatively little further contribution to it, and then continue distributing that product for many years.

The author of this project does not believe that a one-time act of adoption should automatically create an unlimited and perpetual exclusive commercial privilege.

At the same time, the project does not attempt to force commercial users to publish their source code.

Instead, the license takes a simpler approach:

> If a commercial project wants to retain the commercial rights granted by this license, it should remain an actively maintained project.

This is the central principle of the Maintenance-Linked Commercial License.

## Non-Commercial Use

Non-commercial use is intentionally unrestricted by the maintenance requirement.

People may use the project for:

* learning;
* research;
* education;
* experimentation;
* personal projects;
* development;
* modification;
* testing; and
* other non-commercial purposes.

The intention is that the maintenance mechanism should not discourage ordinary users from studying or extending the project.

## Commercial Protection Period

Each Release has a Commercial Protection Period of five years beginning on its Release Date.

During this period, a new commercial project may use that Release commercially only if it accepts the maintenance requirements of the license.

The five-year period is therefore not a five-year expiration date for the software.

It is a period during which the author may require new commercial users to participate in the maintenance model.

## Existing Commercial Projects

A particularly important part of this license is that an existing commercial project does not automatically become maintenance-free merely because five years have passed.

For example:

```text
2026  Release 1.0
      │
2027  Commercial Project A begins using 1.0
      │
2028  Maintenance
      │
2029  Maintenance
      │
2030  Maintenance
      │
2031  Five-year Commercial Protection Period ends
```

Project A does not automatically become maintenance-free in 2031.

If Project A wishes to continue relying on the commercial rights it obtained during the protected period, it must continue satisfying the maintenance requirement.

This prevents a commercial project from obtaining a commercial license shortly before the end of the protection period and then retaining an effectively perpetual right by simply waiting.

## New Commercial Users

The situation is different for a project that first adopts an old Release after its Commercial Protection Period has expired.

For example:

```text
2026  Release 1.0
      │
      │
2031  Commercial Protection Period ends
      │
2032  Project B first adopts Release 1.0
```

Project B does not acquire a new maintenance obligation merely by adopting Release 1.0 at that point.

This provides a natural transition for older versions.

The author therefore does not have to maintain commercial restrictions on a particular Release forever.

## What Counts as Maintenance?

The license deliberately does not attempt to define how much code a commercial user must change.

A requirement such as "modify at least 100 lines" or "upgrade to the latest version" would create artificial incentives and would be difficult to apply consistently.

Instead, the license uses the concept of a **Maintenance Event**.

A Maintenance Event can include:

* recompiling the software;
* running and reviewing tests;
* adapting the project to a new compiler or operating system;
* updating dependencies;
* fixing bugs;
* addressing security issues;
* reviewing compatibility;
* updating documentation;
* modifying configuration;
* updating to a newer Release;
* making small source-code changes; or
* another genuine activity showing that the project is still being actively maintained.

A Maintenance Event does not need to produce a large source-code diff.

For example, a company may spend several days making sure an old version still builds correctly on a new system and ultimately commit no source-code changes. That can still constitute meaningful maintenance.

The purpose is not to demand artificial development.

The purpose is to prevent:

> "We integrated it once ten years ago and nobody has looked at it since."

from becoming a permanent basis for commercial distribution rights.

## What Happens If Maintenance Stops?

If a commercial project goes without a qualifying Maintenance Event for the period specified by the license, its commercial rights under the Maintenance-Linked Commercial License terminate.

This does **not** mean that the commercial user must publish its source code.

The license does not contain a source-disclosure requirement.

Instead, the commercial user must stop relying on this license to continue commercially distributing the affected work.

In practical terms, a commercial project that wants to continue its distribution rights must either:

1. resume and satisfy the applicable licensing requirements where permitted;
2. obtain another commercial license from the copyright holder;
3. use a version or component for which it has an independent right of commercial use; or
4. stop the affected commercial distribution.

## What This License Is Trying to Encourage

The intended incentive is therefore straightforward:

**Use the project freely for non-commercial purposes.**

**If you build a commercial product around it, keep the project alive.**

**If the project becomes old enough, new commercial users eventually receive unrestricted commercial access.**

**If an existing commercial user wants to retain its special commercial rights, it must continue actively maintaining the project rather than merely waiting for time to pass.**

This creates a gradual transition rather than permanent commercial exclusivity.

## Relationship to Rogue

This license does not attempt to change the license of the original Rogue source code.

The original Rogue material remains under its original license.

The Maintenance-Linked Commercial License applies only to original contributions for which the project author owns the relevant rights and is able to grant this license.

Third-party material remains under its own license.

The project therefore deliberately maintains a separation between:

* historical Rogue material;
* new original contributions; and
* third-party components.

See `LICENSE-NOTICE.md` for the project's licensing and provenance summary.

## Legal Status

This is a custom software license designed for this project.

It is **not** intended to be represented as an OSI-approved Open Source license or as the Business Source License.

Existing licenses such as the Business Source License use a different model in which a version's rights transition to a specified Change License on a defined Change Date.

This license instead attempts to connect continuing commercial rights to the maintenance status of the particular commercial project.

Because custom software licenses can have different legal effects depending on jurisdiction and the exact facts of a distribution, this license should be reviewed by a qualified attorney before being relied upon for significant commercial distribution.

## Philosophy

The underlying principle is simple:

> **Commercial use should be easy to obtain, but perpetual commercial rights should be accompanied by continuing responsibility.**

The goal is not to punish commercial users or force them to open-source their work.

The goal is to avoid a situation in which a project is commercially valuable enough to support a closed-source product, while the upstream project has effectively been abandoned by everyone who commercially depends on it.

A commercial user that continues to maintain its project should be able to continue using it.

A commercial user that abandons the project indefinitely should eventually lose the special commercial rights granted by this license.

And older versions should gradually become easier for new users to adopt without creating an indefinite licensing burden for the original author.
