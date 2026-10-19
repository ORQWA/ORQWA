package org.torqa.android;

import android.Manifest;
import android.app.Activity;
import android.os.Build;
import android.view.View;

import org.godotengine.godot.Godot;
import org.godotengine.godot.plugin.GodotPlugin;

/**
 * Readies Torqa on Android (ADR 0024). Godot creates its plugins before it opens its
 * extensions, on the app's main thread: loading Torqa's library here runs its JNI_OnLoad with
 * the app's class loader, the one moment btleplug can find its Java classes. Then it asks for
 * Bluetooth, which Android grants only at run time.
 */
public class TorqaAndroid extends GodotPlugin {
    public TorqaAndroid(Godot godot) {
        super(godot);
        System.loadLibrary("torqa_gd");
    }

    @Override
    public String getPluginName() {
        return "TorqaAndroid";
    }

    @Override
    public View onMainCreate(Activity activity) {
        String[] bluetooth = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
                ? new String[] {Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT}
                : new String[] {Manifest.permission.ACCESS_FINE_LOCATION};
        activity.requestPermissions(bluetooth, 1);
        return null;
    }
}
