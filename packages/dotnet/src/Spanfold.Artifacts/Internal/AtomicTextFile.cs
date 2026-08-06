namespace Spanfold.Artifacts.Internal;

internal static class AtomicTextFile
{
    internal static void Write(string path, string content)
    {
        var directory = Path.GetDirectoryName(path)!;
        var temporary = Path.Combine(
            directory,
            "." + Path.GetFileName(path) + ".tmp-" + Guid.NewGuid().ToString("N"));

        try
        {
            File.WriteAllText(temporary, content);
            File.Move(temporary, path, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporary))
            {
                File.Delete(temporary);
            }
        }
    }
}
