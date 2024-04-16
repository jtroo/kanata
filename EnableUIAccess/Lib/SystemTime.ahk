/*
    SystemTime - Wrapper for Win32 SYSTEMTIME Structure
        http://msdn.microsoft.com/en-us/library/ms724950

    Usage Examples:

    ; Create structure from string.
    st := SystemTime.FromString(A_Now)
    
    ; Shortcut:
    st := SystemTime.Now()

    ; Update values.
    st.FromString(A_Now)
    
    ; Retrieve components.
    year    := st.Year
    month   := st.Month
    weekday := st.DayOfWeek
    day     := st.Day
    hour    := st.Hour
    minute  := st.Minute
    second  := st.Second
    ms      := st.Milliseconds
    
    ; Set or perform math on component.
    st.Year += 10

    ; Create structure to receive output from DllCall.
    st := new SystemTime
    DllCall("GetSystemTime", "ptr", st.p)
    MsgBox % st.ToString()

    ; Fill external structure.
    st := SystemTime.FromPointer(externalPointer)
    st.FromString(A_Now)

    ; Convert external structure to string.
    MsgBox % SystemTime.ToString(externalPointer)

*/

class SystemTime
{
    FromString(str)
    {
        if this.p
            st := this
        else
            st := new this
        if !(p := st.p)
            return 0
        FormatTime wday, %str%, WDay
        wday -= 1
        FormatTime str, %str%, yyyy M '%wday%' d H m s '0'
        Loop Parse, str, %A_Space%
            NumPut(A_LoopField, p+(A_Index-1)*2, "ushort")
        return st
    }
    
    FromPointer(pointer)
    {
        return { p: pointer, base: this }   ; Does not call __New.
    }

    ToString(st = 0)
    {
        if !(p := (st ? (IsObject(st) ? st.p : st) : this.p))
            return ""
        VarSetCapacity(s, 28), s := SubStr("000" NumGet(p+0, "ushort"), -3)
        Loop 6
            if A_Index != 2
                s .= SubStr("0" NumGet(p+A_Index*2, "ushort"), -1)
        return s
    }
    
    Now()
    {
        return this.FromString(A_Now)
    }

    __New()
    {
        if !(this.SetCapacity("struct", 16))
        || !(this.p := this.GetAddress("struct"))
            return 0
        NumPut(0, NumPut(0, this.p, "int64"), "int64")
    }
    
    __GetSet(name, value="")
    {
        static fields := {Year:0, Month:2, DayOfWeek:4, Day:6, Hour:8
                            , Minute:10, Second:12, Milliseconds:14}
        if fields.HasKey(name)
            return value=""
                ? NumGet(       this.p + fields[name], "ushort")
                : NumPut(value, this.p + fields[name], "ushort")
    }
    static __Get := SystemTime.__GetSet
    static __Set := SystemTime.__GetSet
}