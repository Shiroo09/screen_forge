using System.IO;
using System.IO.Pipes;
using System.Text;

namespace ScreenForge.Background;

public static class BackgroundEditBridge
{
    private const string PipeName = "ScreenForge.BackgroundEdit.v1";

    public static bool TrySend(string path)
    {
        try
        {
            using var client = new NamedPipeClientStream(".", PipeName, PipeDirection.Out);
            client.Connect(1500);
            using var writer = new StreamWriter(client, Encoding.UTF8, leaveOpen: false);
            writer.WriteLine(path);
            return true;
        }
        catch
        {
            return false;
        }
    }

    public static void StartServer(Action<string> onPath)
    {
        var thread = new Thread(() => Listen(onPath))
        {
            IsBackground = true,
            Name = "ScreenForge.BackgroundEdit",
        };
        thread.Start();
    }

    private static void Listen(Action<string> onPath)
    {
        while (true)
        {
            try
            {
                using var server = new NamedPipeServerStream(
                    PipeName,
                    PipeDirection.In,
                    1,
                    PipeTransmissionMode.Byte,
                    PipeOptions.Asynchronous);
                server.WaitForConnection();
                using var reader = new StreamReader(server, Encoding.UTF8);
                var path = reader.ReadLine();
                if (!string.IsNullOrWhiteSpace(path))
                    onPath(path.Trim());
            }
            catch
            {
                Thread.Sleep(200);
            }
        }
    }
}
