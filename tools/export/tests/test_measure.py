from export.measure import peak_working_set_bytes


def test_peak_working_set_reports_a_real_value():
    # The Python process alone holds more than 1 MiB. A zero means the
    # platform call failed silently.
    assert peak_working_set_bytes() > 1 << 20
