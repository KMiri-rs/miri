---
name: PR-description
description: Write clear GitHub pull request descriptions for AI-authored changes, including bug fixes, cross-repository issue links, sibling PRs, design decisions, validation, and diagnostic output.
---

# Write a pull request description

Use this skill when preparing or revising a GitHub pull request description. Match the body to the change, make issue and pull request links unambiguous across repositories, and keep detailed explanations and logs collapsible.

## Choose the target branch for KMiri-rs repositories

Almost all repositories in the `KMiri-rs` organization are forks and maintain a `kmiri` branch. When opening a pull request to a `KMiri-rs` repository, target its `kmiri` branch. If that branch does not exist in the target repository, use the repository's remote default branch.

## Qualify every GitHub issue and pull request number

Always write references as `user/repo#number`, including references to an issue or pull request in the current repository. Never use a bare `#number`: in a fork or a multi-repository change, GitHub may resolve it to the wrong repository and render a misleading link.

Put the qualified reference immediately after a space when possible so GitHub recognizes and renders it. For example, write `Fixes KMiri-rs/KMiri#123` or `sibling PR: a/b#1`; do not attach punctuation or other characters directly before the reference. Keep the repository owner and name exactly as GitHub identifies them.

## Avoid source line numbers in prose

Outside code snippets and diagnostic output, describe the relevant file, module, or component without pointing to a specific line. If a line number is needed to help the reader find the code, make it a link to a GitHub permalink at a commit SHA, using this form:

```markdown
[src/example.rs:42](https://github.com/owner/repo/blob/<commit-sha>/src/example.rs#L42)
```

Use the actual repository, commit SHA, file path, and line anchor. Do not use a branch-based URL for this purpose, since the target can move.

## Bug-fix pull requests

Start with the issue-closing information when the change fixes an open issue. Use GitHub's closing keyword and the fully qualified issue reference:

```text
close user/repo#number
```

When multiple repositories need sibling pull requests to resolve one issue, list every related pull request with a fully qualified reference on that first line:

```text
close user/repo#number with sibling PR a/b#1 , a/c#1
```

Omit the closing line when there is no related open issue. Do not use a bare issue or pull request number anywhere else in the description either.

After the closing line, use a level-three heading for each section and put that section's details in an HTML `<details>` block. Cover:

1. **Background and bug** — the context and the incorrect behavior.
2. **Root cause** — why the bug occurred.
3. **Reproduction** — the steps, setup, and expected versus actual behavior.
4. **Fix** — the solution, the main modules or components changed, and how the change works with sibling pull requests.
5. **Validation** — how the fix was checked and what demonstrates that the bug is resolved.
6. **Output before the fix** and **Output after the fix** — include the original, unedited outputs that show the failure and the corrected behavior. Put diagnostic output inside fenced code blocks within the details block.

Use this structure, omitting a section only when it does not apply. Never fabricate output; if original before-and-after output is unavailable, say so in the relevant section and summarize the available evidence instead.

````markdown
close user/repo#number with sibling PR a/b#1 , a/c#1

### Background and bug
<details>
<summary>Background and behavior</summary>

Describe the context and incorrect behavior.

</details>

### Root cause
<details>
<summary>Cause</summary>

Explain the underlying cause.

</details>

### Reproduction
<details>
<summary>Steps and observed behavior</summary>

Describe setup, steps, expected behavior, and actual behavior.

</details>

### Fix
<details>
<summary>Implementation</summary>

Explain the approach, changed modules, and sibling PR relationship.

</details>

### Validation
<details>
<summary>Checks</summary>

Describe the checks and their results.

</details>

### Output before the fix
<details>
<summary>Original failing output</summary>

```text
Paste the original output here.
```

</details>

### Output after the fix
<details>
<summary>Original output after the fix</summary>

```text
Paste the original output here.
```

</details>
````

If the PR does not close an issue but does have related sibling PRs, mention those fully qualified references in the **Fix** section.

## Feature pull requests

For a feature, start with a closing line only if it closes an issue, using the same qualified format. Then describe the feature design and user-visible behavior, the important implementation choices, and how it was validated. Use level-three headings followed by HTML `<details>` blocks for these sections. Include only sections useful for understanding the change, and keep long logs or diagnostic output inside fenced code blocks within a details block.

Example outline:

````markdown
close user/repo#number

### Design
<details>
<summary>Feature design</summary>

Explain the intended behavior and key design choices.

</details>

### Validation
<details>
<summary>Checks</summary>

Describe how the feature was checked and the result.

</details>
````

## Final review

- For a pull request to a `KMiri-rs` repository, target `kmiri` if it exists; otherwise target the remote default branch.
- Check that every GitHub issue, discussion, and pull request reference is written as `user/repo#number`.
- Avoid specific source line numbers in prose; if one is needed, link it to a commit permalink.
- Confirm that a closing line names the issue the PR actually resolves, and list sibling PRs when they jointly resolve it.
- Keep all explanation sections under level-three headings with details folded in `<details>` blocks.
- Preserve original diagnostic output exactly and fold it inside details blocks.
- Keep the description focused on the change and its evidence; do not claim validation that was not performed.
