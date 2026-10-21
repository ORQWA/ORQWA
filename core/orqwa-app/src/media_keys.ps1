$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class OrqwaMediaKeys
{
    [StructLayout(LayoutKind.Sequential)]
    public struct Input
    {
        public uint Type;
        public InputData Data;
    }

    [StructLayout(LayoutKind.Explicit)]
    public struct InputData
    {
        [FieldOffset(0)] public KeyboardInput Keyboard;
        // INPUT's union must include its largest member, even for keyboard events.
        [FieldOffset(0)] public MouseInput Mouse;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct KeyboardInput
    {
        public ushort VirtualKey;
        public ushort ScanCode;
        public uint Flags;
        public uint Time;
        public UIntPtr ExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct MouseInput
    {
        public int X;
        public int Y;
        public uint MouseData;
        public uint Flags;
        public uint Time;
        public UIntPtr ExtraInfo;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(uint count, Input[] inputs, int size);

    public static Input[] Events(ushort key)
    {
        Input down = new Input();
        down.Type = 1; // INPUT_KEYBOARD
        down.Data.Keyboard.VirtualKey = key;
        down.Data.Keyboard.Flags = 1; // KEYEVENTF_EXTENDEDKEY
        Input up = down;
        up.Data.Keyboard.Flags |= 2; // KEYEVENTF_KEYUP
        return new Input[] { down, up };
    }

    public static void Press(ushort key)
    {
        Input[] inputs = Events(key);
        if (SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(Input))) != inputs.Length)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(),
                "Windows did not accept both media-key events.");
        }
    }
}
'@
