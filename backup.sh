mkdir -p /tmp/backup

git diff-tree --no-commit-id --name-only -r e6fde999385a5bbc79f035679648144e5ae2b9e1 | while read file; do
  mkdir -p "/tmp/backup/$(dirname "$file")"
  git show "e6fde999385a5bbc79f035679648144e5ae2b9e1:$file" > "/tmp/backup/$file"
done
